//! Canonical bounded encoding for one Vellum federation replica checkpoint.

use crate::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, ReplicaCheckpointV1,
    ReplicaJournalV1, ReplicaLimitsV1,
};
use vellum_federation_protocol::{
    DisclosureProfileV1, FederatedWorkspaceRefV1, HomeLocalWorkspaceIdV1, VellumHomeIdV1,
};

const MAGIC: &[u8; 8] = b"VLFRCP01";
const VERSION: u16 = 1;
const FIXED_BYTES: usize = 105;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointWireRefusalV1 {
    Length,
    Magic,
    Version,
    InvalidIdentity,
    InvalidTag,
    Limit,
    Malformed,
}

pub fn encode_checkpoint_v1(
    checkpoint: &ReplicaCheckpointV1,
    limits: ReplicaLimitsV1,
) -> Result<Vec<u8>, CheckpointWireRefusalV1> {
    ReplicaJournalV1::recover(checkpoint.clone(), limits)
        .map_err(|_| CheckpointWireRefusalV1::Malformed)?;
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&VERSION.to_be_bytes());
    output.extend_from_slice(&checkpoint.subscriber.as_bytes());
    output.extend_from_slice(&checkpoint.workspace.home.as_bytes());
    output.extend_from_slice(&checkpoint.workspace.workspace.as_bytes());
    output.extend_from_slice(&(checkpoint.disclosure as u16).to_be_bytes());
    output.extend_from_slice(&checkpoint.next_sequence.to_be_bytes());
    match &checkpoint.snapshot {
        Some(snapshot) => {
            output.push(1);
            output.extend_from_slice(&snapshot.base_sequence.to_be_bytes());
            put_payload(&mut output, snapshot.bytes())?;
        }
        None => output.push(0),
    }
    let count =
        u32::try_from(checkpoint.events.len()).map_err(|_| CheckpointWireRefusalV1::Limit)?;
    output.extend_from_slice(&count.to_be_bytes());
    for event in &checkpoint.events {
        output.extend_from_slice(&event.sequence.to_be_bytes());
        put_payload(&mut output, event.bytes())?;
    }
    Ok(output)
}

pub fn decode_checkpoint_v1(
    bytes: &[u8],
    limits: ReplicaLimitsV1,
) -> Result<ReplicaCheckpointV1, CheckpointWireRefusalV1> {
    if bytes.len() < FIXED_BYTES {
        return Err(CheckpointWireRefusalV1::Length);
    }
    let mut cursor = 0;
    if take::<8>(bytes, &mut cursor)? != *MAGIC {
        return Err(CheckpointWireRefusalV1::Magic);
    }
    if u16::from_be_bytes(take(bytes, &mut cursor)?) != VERSION {
        return Err(CheckpointWireRefusalV1::Version);
    }
    let subscriber = VellumHomeIdV1::new(take(bytes, &mut cursor)?)
        .ok_or(CheckpointWireRefusalV1::InvalidIdentity)?;
    let workspace = FederatedWorkspaceRefV1 {
        home: VellumHomeIdV1::new(take(bytes, &mut cursor)?)
            .ok_or(CheckpointWireRefusalV1::InvalidIdentity)?,
        workspace: HomeLocalWorkspaceIdV1::new(take(bytes, &mut cursor)?)
            .ok_or(CheckpointWireRefusalV1::InvalidIdentity)?,
    };
    let disclosure = DisclosureProfileV1::from_u16(u16::from_be_bytes(take(bytes, &mut cursor)?))
        .ok_or(CheckpointWireRefusalV1::InvalidTag)?;
    let next_sequence = u64::from_be_bytes(take(bytes, &mut cursor)?);
    let snapshot = match take::<1>(bytes, &mut cursor)?[0] {
        0 => None,
        1 => {
            let base_sequence = u64::from_be_bytes(take(bytes, &mut cursor)?);
            let payload = take_payload(bytes, &mut cursor, limits.max_snapshot_bytes)?;
            Some(
                AuthoritativeWorkspaceSnapshotV1::new(
                    workspace,
                    base_sequence,
                    disclosure,
                    payload,
                )
                .ok_or(CheckpointWireRefusalV1::Malformed)?,
            )
        }
        _ => return Err(CheckpointWireRefusalV1::InvalidTag),
    };
    let event_count = usize::try_from(u32::from_be_bytes(take(bytes, &mut cursor)?))
        .map_err(|_| CheckpointWireRefusalV1::Limit)?;
    if event_count > limits.max_events {
        return Err(CheckpointWireRefusalV1::Limit);
    }
    let mut events = Vec::with_capacity(event_count);
    for _ in 0..event_count {
        let sequence = u64::from_be_bytes(take(bytes, &mut cursor)?);
        let payload = take_payload(bytes, &mut cursor, limits.max_event_bytes)?;
        events.push(
            AuthoritativeWorkspaceEventV1::new(workspace, sequence, disclosure, payload)
                .ok_or(CheckpointWireRefusalV1::Malformed)?,
        );
    }
    if cursor != bytes.len() {
        return Err(CheckpointWireRefusalV1::Length);
    }
    let checkpoint = ReplicaCheckpointV1 {
        subscriber,
        workspace,
        disclosure,
        next_sequence,
        snapshot,
        events,
    };
    ReplicaJournalV1::recover(checkpoint.clone(), limits)
        .map_err(|_| CheckpointWireRefusalV1::Malformed)?;
    Ok(checkpoint)
}

fn put_payload(output: &mut Vec<u8>, payload: &[u8]) -> Result<(), CheckpointWireRefusalV1> {
    let length = u32::try_from(payload.len()).map_err(|_| CheckpointWireRefusalV1::Limit)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(payload);
    Ok(())
}

fn take_payload(
    bytes: &[u8],
    cursor: &mut usize,
    maximum: usize,
) -> Result<Vec<u8>, CheckpointWireRefusalV1> {
    let length = usize::try_from(u32::from_be_bytes(take(bytes, cursor)?))
        .map_err(|_| CheckpointWireRefusalV1::Limit)?;
    if length == 0 || length > maximum {
        return Err(CheckpointWireRefusalV1::Limit);
    }
    let end = cursor
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or(CheckpointWireRefusalV1::Length)?;
    let payload = bytes[*cursor..end].to_vec();
    *cursor = end;
    Ok(payload)
}

fn take<const N: usize>(
    input: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], CheckpointWireRefusalV1> {
    let end = cursor
        .checked_add(N)
        .filter(|end| *end <= input.len())
        .ok_or(CheckpointWireRefusalV1::Length)?;
    let value = input[*cursor..end]
        .try_into()
        .map_err(|_| CheckpointWireRefusalV1::Length)?;
    *cursor = end;
    Ok(value)
}
