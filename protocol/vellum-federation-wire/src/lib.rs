//! Canonical, bounded binary framing for Vellum federation replica traffic.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, FederationCapabilitiesV1, FederationHelloV1, FederationLimitsV1,
    HomeLocalWorkspaceIdV1, ProfileRangeV1, ReplicaAcknowledgementV1, ServiceIncarnationV1,
    TrustEpochV1, VellumHomeIdV1,
};

const MAGIC: &[u8; 4] = b"VFW1";
const EVENT: u8 = 1;
const SNAPSHOT: u8 = 2;
const ACKNOWLEDGEMENT: u8 = 3;
const HEADER_LEN: usize = 63;
const HELLO_MAGIC: &[u8; 4] = b"VFH1";
const HELLO_LEN: usize = 106;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireRefusalV1 {
    FrameTooLarge,
    Truncated,
    BadMagic,
    WrongFrameType,
    InvalidIdentity,
    InvalidDisclosure,
    InvalidSequence,
    InvalidProfile,
    InvalidLimits,
    EmptyPayload,
    PayloadTooLarge,
    LengthMismatch,
}

pub fn encode_hello(hello: FederationHelloV1) -> [u8; HELLO_LEN] {
    let mut frame = [0_u8; HELLO_LEN];
    let mut offset = 0;
    put(&mut frame, &mut offset, HELLO_MAGIC);
    put(&mut frame, &mut offset, &hello.home.as_bytes());
    put(
        &mut frame,
        &mut offset,
        &hello.trust_epoch.get().to_be_bytes(),
    );
    put(&mut frame, &mut offset, &hello.incarnation.as_bytes());
    for range in [
        hello.capabilities.protocol,
        hello.capabilities.disclosure,
        hello.capabilities.event_schema,
        hello.capabilities.snapshot_schema,
        hello.capabilities.edit_schema,
        hello.capabilities.receipt_schema,
    ] {
        put(&mut frame, &mut offset, &range.minimum.to_be_bytes());
        put(&mut frame, &mut offset, &range.maximum.to_be_bytes());
    }
    let limits = hello.capabilities.limits;
    put(
        &mut frame,
        &mut offset,
        &limits.max_frame_bytes.to_be_bytes(),
    );
    put(
        &mut frame,
        &mut offset,
        &limits.max_source_bytes.to_be_bytes(),
    );
    put(&mut frame, &mut offset, &limits.max_documents.to_be_bytes());
    put(&mut frame, &mut offset, &limits.max_in_flight.to_be_bytes());
    put(
        &mut frame,
        &mut offset,
        &limits.max_subscriptions.to_be_bytes(),
    );
    put(
        &mut frame,
        &mut offset,
        &limits.max_replay_events.to_be_bytes(),
    );
    put(
        &mut frame,
        &mut offset,
        &limits.max_snapshot_bytes.to_be_bytes(),
    );
    put(&mut frame, &mut offset, &limits.max_receipts.to_be_bytes());
    debug_assert_eq!(offset, HELLO_LEN);
    frame
}

pub fn decode_hello(frame: &[u8]) -> Result<FederationHelloV1, WireRefusalV1> {
    if frame.len() < HELLO_LEN {
        return Err(WireRefusalV1::Truncated);
    }
    if frame.len() != HELLO_LEN {
        return Err(WireRefusalV1::LengthMismatch);
    }
    if &frame[..4] != HELLO_MAGIC {
        return Err(WireRefusalV1::BadMagic);
    }
    let home = VellumHomeIdV1::new(frame[4..36].try_into().expect("fixed slice"))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let trust_epoch = TrustEpochV1::new(u64::from_be_bytes(
        frame[36..44].try_into().expect("fixed slice"),
    ))
    .ok_or(WireRefusalV1::InvalidIdentity)?;
    let incarnation = ServiceIncarnationV1::new(frame[44..60].try_into().expect("fixed slice"))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let mut offset = 60;
    let protocol = take_range(frame, &mut offset)?;
    let disclosure = take_range(frame, &mut offset)?;
    let event_schema = take_range(frame, &mut offset)?;
    let snapshot_schema = take_range(frame, &mut offset)?;
    let edit_schema = take_range(frame, &mut offset)?;
    let receipt_schema = take_range(frame, &mut offset)?;
    let limits = FederationLimitsV1 {
        max_frame_bytes: take_u32(frame, &mut offset),
        max_source_bytes: take_u32(frame, &mut offset),
        max_documents: take_u16(frame, &mut offset),
        max_in_flight: take_u16(frame, &mut offset),
        max_subscriptions: take_u16(frame, &mut offset),
        max_replay_events: take_u16(frame, &mut offset),
        max_snapshot_bytes: take_u32(frame, &mut offset),
        max_receipts: take_u16(frame, &mut offset),
    };
    if limits.max_frame_bytes == 0
        || limits.max_source_bytes == 0
        || limits.max_documents == 0
        || limits.max_in_flight == 0
        || limits.max_subscriptions == 0
        || limits.max_replay_events == 0
        || limits.max_snapshot_bytes == 0
        || limits.max_receipts == 0
    {
        return Err(WireRefusalV1::InvalidLimits);
    }
    Ok(FederationHelloV1 {
        home,
        trust_epoch,
        incarnation,
        capabilities: FederationCapabilitiesV1 {
            protocol,
            disclosure,
            event_schema,
            snapshot_schema,
            edit_schema,
            receipt_schema,
            limits,
        },
    })
}

fn put<const N: usize>(frame: &mut [u8], offset: &mut usize, bytes: &[u8; N]) {
    frame[*offset..*offset + N].copy_from_slice(bytes);
    *offset += N;
}

fn take_u16(frame: &[u8], offset: &mut usize) -> u16 {
    let value = u16::from_be_bytes(frame[*offset..*offset + 2].try_into().expect("fixed slice"));
    *offset += 2;
    value
}

fn take_u32(frame: &[u8], offset: &mut usize) -> u32 {
    let value = u32::from_be_bytes(frame[*offset..*offset + 4].try_into().expect("fixed slice"));
    *offset += 4;
    value
}

fn take_range(frame: &[u8], offset: &mut usize) -> Result<ProfileRangeV1, WireRefusalV1> {
    ProfileRangeV1::new(take_u16(frame, offset), take_u16(frame, offset))
        .ok_or(WireRefusalV1::InvalidProfile)
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
