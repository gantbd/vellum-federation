//! Injected current-home and disclosure trust for Vellum federation sessions.

#![forbid(unsafe_code)]

use core::fmt;
use vellum_federation_protocol::{DisclosureProfileV1, TransportExporterV1, VerifiedHomeV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederationTrustRoleV1 {
    Local,
    Peer,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FederationTrustRequestV1 {
    pub role: FederationTrustRoleV1,
    pub claimed: VerifiedHomeV1,
    pub exporter: TransportExporterV1,
    pub disclosure: DisclosureProfileV1,
}

impl fmt::Debug for FederationTrustRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederationTrustRequestV1")
            .field("role", &self.role)
            .field("claimed", &"<redacted>")
            .field("exporter", &"<redacted>")
            .field("disclosure", &self.disclosure)
            .finish()
    }
}

pub trait FederationTrustPortV1 {
    type Context;
    type Error;

    fn verify(
        &mut self,
        request: FederationTrustRequestV1,
        context: &Self::Context,
    ) -> Result<VerifiedHomeV1, Self::Error>;
}
