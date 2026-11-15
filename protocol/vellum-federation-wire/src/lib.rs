//! Canonical, bounded binary framing for Vellum federation replica traffic.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, HomeLocalWorkspaceIdV1, ReplicaAcknowledgementV1, VellumHomeIdV1,
};

const MAGIC: &[u8; 4] = b"VFW1";
const EVENT: u8 = 1;
const SNAPSHOT: u8 = 2;
const ACKNOWLEDGEMENT: u8 = 3;
const HEADER_LEN: usize = 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireRefusalV1 {
    FrameTooLarge,
    Truncated,
    BadMagic,
    WrongFrameType,
    InvalidIdentity,
    InvalidDisclosure,
    InvalidSequence,
    EmptyPayload,
    PayloadTooLarge,
    LengthMismatch,
}

pub fn encode_event(event: &AuthoritativeWorkspaceEventV1) -> Result<Vec<u8>, WireRefusalV1> {
    encode_payload(
        EVENT,
        event.workspace,
        event.disclosure,
        event.sequence,
        event.bytes(),
    )
}

pub fn decode_event(
    frame: &[u8],
    max_payload_bytes: usize,
) -> Result<AuthoritativeWorkspaceEventV1, WireRefusalV1> {
    let decoded = decode_payload(frame, EVENT, max_payload_bytes)?;
    AuthoritativeWorkspaceEventV1::new(
        decoded.workspace,
        decoded.sequence,
        decoded.disclosure,
        decoded.payload,
    )
    .ok_or(WireRefusalV1::InvalidSequence)
}

pub fn encode_snapshot(
    snapshot: &AuthoritativeWorkspaceSnapshotV1,
) -> Result<Vec<u8>, WireRefusalV1> {
    encode_payload(
        SNAPSHOT,
        snapshot.workspace,
        snapshot.disclosure,
        snapshot.base_sequence,
        snapshot.bytes(),
    )
}

pub fn decode_snapshot(
    frame: &[u8],
    max_payload_bytes: usize,
) -> Result<AuthoritativeWorkspaceSnapshotV1, WireRefusalV1> {
    let decoded = decode_payload(frame, SNAPSHOT, max_payload_bytes)?;
    AuthoritativeWorkspaceSnapshotV1::new(
        decoded.workspace,
        decoded.sequence,
        decoded.disclosure,
        decoded.payload,
    )
    .ok_or(WireRefusalV1::InvalidSequence)
}

pub fn encode_acknowledgement(
    acknowledgement: ReplicaAcknowledgementV1,
    disclosure: DisclosureProfileV1,
) -> Vec<u8> {
    let mut frame = header(
        ACKNOWLEDGEMENT,
        acknowledgement.workspace,
        disclosure,
        acknowledgement.sequence,
    );
    frame.extend_from_slice(&0_u32.to_be_bytes());
    frame
}

pub fn decode_acknowledgement(
    frame: &[u8],
) -> Result<(ReplicaAcknowledgementV1, DisclosureProfileV1), WireRefusalV1> {
    let decoded = decode_payload(frame, ACKNOWLEDGEMENT, 0)?;
    if !decoded.payload.is_empty() {
        return Err(WireRefusalV1::LengthMismatch);
    }
    Ok((
        ReplicaAcknowledgementV1 {
            workspace: decoded.workspace,
            sequence: decoded.sequence,
        },
        decoded.disclosure,
    ))
}

fn encode_payload(
    kind: u8,
    workspace: FederatedWorkspaceRefV1,
    disclosure: DisclosureProfileV1,
    sequence: u64,
    payload: &[u8],
) -> Result<Vec<u8>, WireRefusalV1> {
    let length = u32::try_from(payload.len()).map_err(|_| WireRefusalV1::FrameTooLarge)?;
    let mut frame = header(kind, workspace, disclosure, sequence);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn header(
    kind: u8,
    workspace: FederatedWorkspaceRefV1,
    disclosure: DisclosureProfileV1,
    sequence: u64,
) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN);
    frame.extend_from_slice(MAGIC);
    frame.push(kind);
    frame.extend_from_slice(&workspace.home.as_bytes());
    frame.extend_from_slice(&workspace.workspace.as_bytes());
    frame.extend_from_slice(&disclosure.as_u16().to_be_bytes());
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame
}

struct DecodedPayload {
    workspace: FederatedWorkspaceRefV1,
    disclosure: DisclosureProfileV1,
    sequence: u64,
    payload: Vec<u8>,
}

fn decode_payload(
    frame: &[u8],
    expected_kind: u8,
    max_payload_bytes: usize,
) -> Result<DecodedPayload, WireRefusalV1> {
    if frame.len() < HEADER_LEN {
        return Err(WireRefusalV1::Truncated);
    }
    if &frame[..4] != MAGIC {
        return Err(WireRefusalV1::BadMagic);
    }
    if frame[4] != expected_kind {
        return Err(WireRefusalV1::WrongFrameType);
    }
    let home = VellumHomeIdV1::new(frame[5..37].try_into().expect("fixed slice"))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let workspace = HomeLocalWorkspaceIdV1::new(frame[37..53].try_into().expect("fixed slice"))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let disclosure = DisclosureProfileV1::from_u16(u16::from_be_bytes(
        frame[53..55].try_into().expect("fixed slice"),
    ))
    .ok_or(WireRefusalV1::InvalidDisclosure)?;
    let sequence = u64::from_be_bytes(frame[55..63].try_into().expect("fixed slice"));
    if sequence == 0 {
        return Err(WireRefusalV1::InvalidSequence);
    }
    if frame.len() < HEADER_LEN + 4 {
        return Err(WireRefusalV1::Truncated);
    }
    let payload_len = u32::from_be_bytes(frame[63..67].try_into().expect("fixed slice")) as usize;
    if payload_len > max_payload_bytes {
        return Err(WireRefusalV1::PayloadTooLarge);
    }
    if frame.len() != HEADER_LEN + 4 + payload_len {
        return Err(WireRefusalV1::LengthMismatch);
    }
    if expected_kind != ACKNOWLEDGEMENT && payload_len == 0 {
        return Err(WireRefusalV1::EmptyPayload);
    }
    Ok(DecodedPayload {
        workspace: FederatedWorkspaceRefV1 { home, workspace },
        disclosure,
        sequence,
        payload: frame[67..].to_vec(),
    })
}
