//! Closed identities and bounded negotiation for native Vellum federation.
//!
//! These values grant no workspace, source, edit, save, or persistence authority.

#![forbid(unsafe_code)]

use core::fmt;

macro_rules! redacted_nonzero_id {
    ($name:ident, $size:literal) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $size]);

        impl $name {
            pub fn new(bytes: [u8; $size]) -> Option<Self> {
                (bytes != [0; $size]).then_some(Self(bytes))
            }

            pub fn as_bytes(self) -> [u8; $size] {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

redacted_nonzero_id!(VellumHomeIdV1, 32);
redacted_nonzero_id!(HomeLocalWorkspaceIdV1, 16);
redacted_nonzero_id!(HomeLocalDocumentIdV1, 16);
redacted_nonzero_id!(ServiceIncarnationV1, 16);
redacted_nonzero_id!(TransportExporterV1, 32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustEpochV1(u64);

impl TrustEpochV1 {
    pub fn new(value: u64) -> Option<Self> {
        (value != 0).then_some(Self(value))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedHomeV1 {
    pub home: VellumHomeIdV1,
    pub trust_epoch: TrustEpochV1,
}

impl fmt::Debug for VerifiedHomeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedHomeV1")
            .field("home", &self.home)
            .field("trust_epoch", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevisionV1(u64);

impl SourceRevisionV1 {
    pub fn new(value: u64) -> Option<Self> {
        (value != 0).then_some(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FederatedWorkspaceRefV1 {
    pub home: VellumHomeIdV1,
    pub workspace: HomeLocalWorkspaceIdV1,
}

impl fmt::Debug for FederatedWorkspaceRefV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederatedWorkspaceRefV1")
            .field("home", &self.home)
            .field("workspace", &self.workspace)
            .finish()
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FederatedDocumentRefV1 {
    pub workspace: FederatedWorkspaceRefV1,
    pub document: HomeLocalDocumentIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeWorkspaceEventV1 {
    pub workspace: FederatedWorkspaceRefV1,
    pub sequence: u64,
    pub disclosure: DisclosureProfileV1,
    bytes: Vec<u8>,
}

impl AuthoritativeWorkspaceEventV1 {
    pub fn new(
        workspace: FederatedWorkspaceRefV1,
        sequence: u64,
        disclosure: DisclosureProfileV1,
        bytes: Vec<u8>,
    ) -> Option<Self> {
        (sequence != 0 && !bytes.is_empty()).then_some(Self {
            workspace,
            sequence,
            disclosure,
            bytes,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeWorkspaceSnapshotV1 {
    pub workspace: FederatedWorkspaceRefV1,
    pub base_sequence: u64,
    pub disclosure: DisclosureProfileV1,
    bytes: Vec<u8>,
}

impl AuthoritativeWorkspaceSnapshotV1 {
    pub fn new(
        workspace: FederatedWorkspaceRefV1,
        base_sequence: u64,
        disclosure: DisclosureProfileV1,
        bytes: Vec<u8>,
    ) -> Option<Self> {
        (base_sequence != 0 && !bytes.is_empty()).then_some(Self {
            workspace,
            base_sequence,
            disclosure,
            bytes,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplicaAcknowledgementV1 {
    pub workspace: FederatedWorkspaceRefV1,
    pub sequence: u64,
}

impl fmt::Debug for FederatedDocumentRefV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederatedDocumentRefV1")
            .field("workspace", &self.workspace)
            .field("document", &self.document)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum DisclosureProfileV1 {
    Metadata = 1,
    Source = 2,
}

impl DisclosureProfileV1 {
    fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Metadata),
            2 => Some(Self::Source),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileRangeV1 {
    pub minimum: u16,
    pub maximum: u16,
}

impl ProfileRangeV1 {
    pub fn new(minimum: u16, maximum: u16) -> Option<Self> {
        (minimum != 0 && minimum <= maximum).then_some(Self { minimum, maximum })
    }

    fn highest_common(self, other: Self) -> Option<u16> {
        let minimum = self.minimum.max(other.minimum);
        let maximum = self.maximum.min(other.maximum);
        (minimum <= maximum).then_some(maximum)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FederationLimitsV1 {
    pub max_frame_bytes: u32,
    pub max_source_bytes: u32,
    pub max_documents: u16,
    pub max_in_flight: u16,
    pub max_subscriptions: u16,
    pub max_replay_events: u16,
    pub max_snapshot_bytes: u32,
    pub max_receipts: u16,
}

impl FederationLimitsV1 {
    fn valid(self) -> bool {
        self.max_frame_bytes != 0
            && self.max_source_bytes != 0
            && self.max_documents != 0
            && self.max_in_flight != 0
            && self.max_subscriptions != 0
            && self.max_replay_events != 0
            && self.max_snapshot_bytes != 0
            && self.max_receipts != 0
    }

    fn bounded_minimum(self, other: Self) -> Self {
        Self {
            max_frame_bytes: self.max_frame_bytes.min(other.max_frame_bytes),
            max_source_bytes: self.max_source_bytes.min(other.max_source_bytes),
            max_documents: self.max_documents.min(other.max_documents),
            max_in_flight: self.max_in_flight.min(other.max_in_flight),
            max_subscriptions: self.max_subscriptions.min(other.max_subscriptions),
            max_replay_events: self.max_replay_events.min(other.max_replay_events),
            max_snapshot_bytes: self.max_snapshot_bytes.min(other.max_snapshot_bytes),
            max_receipts: self.max_receipts.min(other.max_receipts),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FederationCapabilitiesV1 {
    pub protocol: ProfileRangeV1,
    pub disclosure: ProfileRangeV1,
    pub event_schema: ProfileRangeV1,
    pub snapshot_schema: ProfileRangeV1,
    pub edit_schema: ProfileRangeV1,
    pub receipt_schema: ProfileRangeV1,
    pub limits: FederationLimitsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NegotiatedFederationV1 {
    pub protocol: u16,
    pub disclosure: DisclosureProfileV1,
    pub event_schema: u16,
    pub snapshot_schema: u16,
    pub edit_schema: u16,
    pub receipt_schema: u16,
    pub limits: FederationLimitsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FederationHelloV1 {
    pub home: VellumHomeIdV1,
    pub trust_epoch: TrustEpochV1,
    pub incarnation: ServiceIncarnationV1,
    pub capabilities: FederationCapabilitiesV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FederationSessionV1 {
    pub local: VerifiedHomeV1,
    pub peer: VerifiedHomeV1,
    pub local_incarnation: ServiceIncarnationV1,
    pub peer_incarnation: ServiceIncarnationV1,
    pub exporter: TransportExporterV1,
    pub negotiated: NegotiatedFederationV1,
}

impl fmt::Debug for FederationSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederationSessionV1")
            .field("local", &self.local)
            .field("peer", &self.peer)
            .field("local_incarnation", &self.local_incarnation)
            .field("peer_incarnation", &self.peer_incarnation)
            .field("exporter", &self.exporter)
            .field("negotiated", &self.negotiated)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRefusalV1 {
    LocalIdentityMismatch,
    PeerIdentityMismatch,
    SelfSession,
    Negotiation(NegotiationRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NegotiationRefusalV1 {
    InvalidLimits,
    Protocol,
    Disclosure,
    EventSchema,
    SnapshotSchema,
    EditSchema,
    ReceiptSchema,
}

pub fn negotiate_v1(
    local: FederationCapabilitiesV1,
    peer: FederationCapabilitiesV1,
) -> Result<NegotiatedFederationV1, NegotiationRefusalV1> {
    if !local.limits.valid() || !peer.limits.valid() {
        return Err(NegotiationRefusalV1::InvalidLimits);
    }
    let disclosure = local
        .disclosure
        .highest_common(peer.disclosure)
        .and_then(DisclosureProfileV1::from_u16)
        .ok_or(NegotiationRefusalV1::Disclosure)?;
    Ok(NegotiatedFederationV1 {
        protocol: local
            .protocol
            .highest_common(peer.protocol)
            .ok_or(NegotiationRefusalV1::Protocol)?,
        disclosure,
        event_schema: local
            .event_schema
            .highest_common(peer.event_schema)
            .ok_or(NegotiationRefusalV1::EventSchema)?,
        snapshot_schema: local
            .snapshot_schema
            .highest_common(peer.snapshot_schema)
            .ok_or(NegotiationRefusalV1::SnapshotSchema)?,
        edit_schema: local
            .edit_schema
            .highest_common(peer.edit_schema)
            .ok_or(NegotiationRefusalV1::EditSchema)?,
        receipt_schema: local
            .receipt_schema
            .highest_common(peer.receipt_schema)
            .ok_or(NegotiationRefusalV1::ReceiptSchema)?,
        limits: local.limits.bounded_minimum(peer.limits),
    })
}

pub fn bind_session_v1(
    local: VerifiedHomeV1,
    peer: VerifiedHomeV1,
    local_hello: FederationHelloV1,
    peer_hello: FederationHelloV1,
    exporter: TransportExporterV1,
) -> Result<FederationSessionV1, SessionRefusalV1> {
    if local.home != local_hello.home || local.trust_epoch != local_hello.trust_epoch {
        return Err(SessionRefusalV1::LocalIdentityMismatch);
    }
    if peer.home != peer_hello.home || peer.trust_epoch != peer_hello.trust_epoch {
        return Err(SessionRefusalV1::PeerIdentityMismatch);
    }
    if local.home == peer.home {
        return Err(SessionRefusalV1::SelfSession);
    }
    let negotiated = negotiate_v1(local_hello.capabilities, peer_hello.capabilities)
        .map_err(SessionRefusalV1::Negotiation)?;
    Ok(FederationSessionV1 {
        local,
        peer,
        local_incarnation: local_hello.incarnation,
        peer_incarnation: peer_hello.incarnation,
        exporter,
        negotiated,
    })
}
