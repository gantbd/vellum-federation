//! Configured certificate-provenance and disclosure trust for Vellum federation.

#![forbid(unsafe_code)]

use core::fmt;
use vellum_federation_protocol::{DisclosureProfileV1, TransportExporterV1, VerifiedHomeV1};
use vellum_federation_trust_port::{
    FederationTrustPortV1, FederationTrustRequestV1, FederationTrustRoleV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerProvenanceV1 {
    EncryptedChannel,
    VerifiedCertificate,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredTrustContextV1 {
    local: VerifiedHomeV1,
    peer: VerifiedHomeV1,
    exporter: TransportExporterV1,
    maximum_disclosure: DisclosureProfileV1,
    peer_provenance: PeerProvenanceV1,
}

impl ConfiguredTrustContextV1 {
    pub const fn new(
        local: VerifiedHomeV1,
        peer: VerifiedHomeV1,
        exporter: TransportExporterV1,
        maximum_disclosure: DisclosureProfileV1,
        peer_provenance: PeerProvenanceV1,
    ) -> Self {
        Self {
            local,
            peer,
            exporter,
            maximum_disclosure,
            peer_provenance,
        }
    }
}

impl fmt::Debug for ConfiguredTrustContextV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredTrustContextV1")
            .field("local", &"<redacted>")
            .field("peer", &"<redacted>")
            .field("exporter", &"<redacted>")
            .field("maximum_disclosure", &self.maximum_disclosure)
            .field("peer_provenance", &self.peer_provenance)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfiguredTrustRefusalV1 {
    UnauthenticatedPeer,
    ExporterMismatch,
    DisclosureDenied,
    IdentityMismatch,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ConfiguredTrustV1;

impl FederationTrustPortV1 for ConfiguredTrustV1 {
    type Context = ConfiguredTrustContextV1;
    type Error = ConfiguredTrustRefusalV1;

    fn verify(
        &mut self,
        request: FederationTrustRequestV1,
        context: &Self::Context,
    ) -> Result<VerifiedHomeV1, Self::Error> {
        if context.peer_provenance != PeerProvenanceV1::VerifiedCertificate {
            return Err(ConfiguredTrustRefusalV1::UnauthenticatedPeer);
        }
        if request.exporter != context.exporter {
            return Err(ConfiguredTrustRefusalV1::ExporterMismatch);
        }
        if request.disclosure > context.maximum_disclosure {
            return Err(ConfiguredTrustRefusalV1::DisclosureDenied);
        }
        let expected = match request.role {
            FederationTrustRoleV1::Local => context.local,
            FederationTrustRoleV1::Peer => context.peer,
        };
        if request.claimed != expected {
            return Err(ConfiguredTrustRefusalV1::IdentityMismatch);
        }
        Ok(expected)
    }
}
