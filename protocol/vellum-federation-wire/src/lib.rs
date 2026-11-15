//! Canonical, bounded binary framing for Vellum federation replica traffic.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, FederationCapabilitiesV1, FederationHelloV1, FederationLimitsV1,
    FederationSessionEvidenceV1, HomeLocalDocumentIdV1, HomeLocalWorkspaceIdV1,
    NegotiatedFederationV1, ProfileRangeV1, ReplicaAcknowledgementV1, ServiceIncarnationV1,
    SourceDigestV1, SourceRevisionV1, TrustEpochV1, VellumHomeIdV1, VerifiedHomeV1,
    WorkspaceEventFactV1,
};

const MAGIC: &[u8; 4] = b"VFW1";
const EVENT: u8 = 1;
const SNAPSHOT: u8 = 2;
const ACKNOWLEDGEMENT: u8 = 3;
const HEADER_LEN: usize = 63;
const HELLO_MAGIC: &[u8; 4] = b"VFH1";
const HELLO_LEN: usize = 106;
const SESSION_EVIDENCE_MAGIC: &[u8; 4] = b"VSE1";
const SESSION_EVIDENCE_LEN: usize = 150;

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
    InvalidEvent,
    EmptyPayload,
    PayloadTooLarge,
    LengthMismatch,
}

const DOCUMENT_PRESENT: u8 = 1;
const SOURCE_ADVANCED: u8 = 2;
const DOCUMENT_ABSENT: u8 = 3;
const DOCUMENT_EVENT_LEN: usize = 25;
const SOURCE_ADVANCED_LEN: usize = 65;

/// Encodes one authoritative, content-safe Vellum workspace fact.
pub fn encode_workspace_event_fact(fact: WorkspaceEventFactV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(match fact {
        WorkspaceEventFactV1::SourceAdvanced { .. } => SOURCE_ADVANCED_LEN,
        _ => DOCUMENT_EVENT_LEN,
    });
    match fact {
        WorkspaceEventFactV1::DocumentPresent { document, revision } => {
            bytes.push(DOCUMENT_PRESENT);
            bytes.extend_from_slice(&document.as_bytes());
            bytes.extend_from_slice(&revision.get().to_be_bytes());
        }
        WorkspaceEventFactV1::SourceAdvanced {
            document,
            prior_revision,
            resulting_revision,
            digest,
        } => {
            bytes.push(SOURCE_ADVANCED);
            bytes.extend_from_slice(&document.as_bytes());
            bytes.extend_from_slice(&prior_revision.get().to_be_bytes());
            bytes.extend_from_slice(&resulting_revision.get().to_be_bytes());
            bytes.extend_from_slice(&digest.as_bytes());
        }
        WorkspaceEventFactV1::DocumentAbsent {
            document,
            final_revision,
        } => {
            bytes.push(DOCUMENT_ABSENT);
            bytes.extend_from_slice(&document.as_bytes());
            bytes.extend_from_slice(&final_revision.get().to_be_bytes());
        }
    }
    bytes
}

/// Decodes only the frozen V1 workspace fact vocabulary and exact lengths.
pub fn decode_workspace_event_fact(bytes: &[u8]) -> Result<WorkspaceEventFactV1, WireRefusalV1> {
    let tag = *bytes.first().ok_or(WireRefusalV1::Truncated)?;
    let expected_len = match tag {
        DOCUMENT_PRESENT | DOCUMENT_ABSENT => DOCUMENT_EVENT_LEN,
        SOURCE_ADVANCED => SOURCE_ADVANCED_LEN,
        _ => return Err(WireRefusalV1::InvalidEvent),
    };
    if bytes.len() < expected_len {
        return Err(WireRefusalV1::Truncated);
    }
    if bytes.len() != expected_len {
        return Err(WireRefusalV1::LengthMismatch);
    }
    let document = HomeLocalDocumentIdV1::new(bytes[1..17].try_into().expect("fixed slice"))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let revision = |range: core::ops::Range<usize>| {
        SourceRevisionV1::new(u64::from_be_bytes(
            bytes[range].try_into().expect("fixed slice"),
        ))
        .ok_or(WireRefusalV1::InvalidEvent)
    };
    match tag {
        DOCUMENT_PRESENT => Ok(WorkspaceEventFactV1::DocumentPresent {
            document,
            revision: revision(17..25)?,
        }),
        SOURCE_ADVANCED => WorkspaceEventFactV1::source_advanced(
            document,
            revision(17..25)?,
            revision(25..33)?,
            SourceDigestV1::new(bytes[33..65].try_into().expect("fixed slice"))
                .ok_or(WireRefusalV1::InvalidEvent)?,
        )
        .ok_or(WireRefusalV1::InvalidEvent),
        DOCUMENT_ABSENT => Ok(WorkspaceEventFactV1::DocumentAbsent {
            document,
            final_revision: revision(17..25)?,
        }),
        _ => unreachable!(),
    }
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

pub fn encode_session_evidence(
    evidence: FederationSessionEvidenceV1,
) -> [u8; SESSION_EVIDENCE_LEN] {
    let mut frame = [0_u8; SESSION_EVIDENCE_LEN];
    let mut offset = 0;
    put(&mut frame, &mut offset, SESSION_EVIDENCE_MAGIC);
    for home in [evidence.local, evidence.peer] {
        put(&mut frame, &mut offset, &home.home.as_bytes());
        put(
            &mut frame,
            &mut offset,
            &home.trust_epoch.get().to_be_bytes(),
        );
    }
    put(
        &mut frame,
        &mut offset,
        &evidence.local_incarnation.as_bytes(),
    );
    put(
        &mut frame,
        &mut offset,
        &evidence.peer_incarnation.as_bytes(),
    );
    for profile in [
        evidence.negotiated.protocol,
        evidence.negotiated.disclosure.as_u16(),
        evidence.negotiated.event_schema,
        evidence.negotiated.snapshot_schema,
        evidence.negotiated.edit_schema,
        evidence.negotiated.receipt_schema,
    ] {
        put(&mut frame, &mut offset, &profile.to_be_bytes());
    }
    let limits = evidence.negotiated.limits;
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
    debug_assert_eq!(offset, SESSION_EVIDENCE_LEN);
    frame
}

pub fn decode_session_evidence(frame: &[u8]) -> Result<FederationSessionEvidenceV1, WireRefusalV1> {
    if frame.len() < SESSION_EVIDENCE_LEN {
        return Err(WireRefusalV1::Truncated);
    }
    if frame.len() != SESSION_EVIDENCE_LEN {
        return Err(WireRefusalV1::LengthMismatch);
    }
    if &frame[..4] != SESSION_EVIDENCE_MAGIC {
        return Err(WireRefusalV1::BadMagic);
    }
    let mut offset = 4;
    let mut take_home = || -> Result<VerifiedHomeV1, WireRefusalV1> {
        let home = VellumHomeIdV1::new(take_fixed(frame, &mut offset))
            .ok_or(WireRefusalV1::InvalidIdentity)?;
        let trust_epoch = TrustEpochV1::new(take_u64(frame, &mut offset))
            .ok_or(WireRefusalV1::InvalidIdentity)?;
        Ok(VerifiedHomeV1 { home, trust_epoch })
    };
    let local = take_home()?;
    let peer = take_home()?;
    let local_incarnation = ServiceIncarnationV1::new(take_fixed(frame, &mut offset))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let peer_incarnation = ServiceIncarnationV1::new(take_fixed(frame, &mut offset))
        .ok_or(WireRefusalV1::InvalidIdentity)?;
    let negotiated = NegotiatedFederationV1 {
        protocol: take_u16(frame, &mut offset),
        disclosure: DisclosureProfileV1::from_u16(take_u16(frame, &mut offset))
            .ok_or(WireRefusalV1::InvalidDisclosure)?,
        event_schema: take_u16(frame, &mut offset),
        snapshot_schema: take_u16(frame, &mut offset),
        edit_schema: take_u16(frame, &mut offset),
        receipt_schema: take_u16(frame, &mut offset),
        limits: FederationLimitsV1 {
            max_frame_bytes: take_u32(frame, &mut offset),
            max_source_bytes: take_u32(frame, &mut offset),
            max_documents: take_u16(frame, &mut offset),
            max_in_flight: take_u16(frame, &mut offset),
            max_subscriptions: take_u16(frame, &mut offset),
            max_replay_events: take_u16(frame, &mut offset),
            max_snapshot_bytes: take_u32(frame, &mut offset),
            max_receipts: take_u16(frame, &mut offset),
        },
    };
    FederationSessionEvidenceV1::new(local, peer, local_incarnation, peer_incarnation, negotiated)
        .ok_or(WireRefusalV1::InvalidLimits)
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

fn take_u64(frame: &[u8], offset: &mut usize) -> u64 {
    let value = u64::from_be_bytes(frame[*offset..*offset + 8].try_into().expect("fixed slice"));
    *offset += 8;
    value
}

fn take_fixed<const N: usize>(frame: &[u8], offset: &mut usize) -> [u8; N] {
    let value = frame[*offset..*offset + N].try_into().expect("fixed slice");
    *offset += N;
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
