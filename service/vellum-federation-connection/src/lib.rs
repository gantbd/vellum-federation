//! Irreversible transport lifecycle for one trust- and disclosure-bound session.

#![forbid(unsafe_code)]

use core::fmt;
use vellum_federation_protocol::FederationSessionV1;
use vellum_federation_session::revalidate_session_v1;
use vellum_federation_transport_port::FederationTransportLifecyclePortV1;
use vellum_federation_trust_port::FederationTrustPortV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederationConnectionStateV1 {
    Active,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederationConnectionRefusalV1 {
    Closed,
    TrustOrDisclosureRevoked,
    TrustOrDisclosureRevokedCloseFailed,
    CloseFailed,
}

pub struct FederationConnectionV1<T> {
    session: FederationSessionV1,
    transport: T,
    state: FederationConnectionStateV1,
}

impl<T> FederationConnectionV1<T> {
    pub const fn new(session: FederationSessionV1, transport: T) -> Self {
        Self {
            session,
            transport,
            state: FederationConnectionStateV1::Active,
        }
    }

    pub const fn state(&self) -> FederationConnectionStateV1 {
        self.state
    }

    pub fn session(&self) -> Option<FederationSessionV1> {
        (self.state == FederationConnectionStateV1::Active).then_some(self.session)
    }

    pub const fn transport(&self) -> &T {
        &self.transport
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T: FederationTransportLifecyclePortV1> FederationConnectionV1<T> {
    pub fn revalidate<P: FederationTrustPortV1>(
        &mut self,
        trust: &mut P,
        context: &P::Context,
    ) -> Result<(), FederationConnectionRefusalV1> {
        if self.state == FederationConnectionStateV1::Closed {
            return Err(FederationConnectionRefusalV1::Closed);
        }
        if revalidate_session_v1(trust, context, self.session).is_ok() {
            return Ok(());
        }
        self.state = FederationConnectionStateV1::Closed;
        match self.transport.close() {
            Ok(()) => Err(FederationConnectionRefusalV1::TrustOrDisclosureRevoked),
            Err(_) => Err(FederationConnectionRefusalV1::TrustOrDisclosureRevokedCloseFailed),
        }
    }

    pub fn close(&mut self) -> Result<(), FederationConnectionRefusalV1> {
        if self.state == FederationConnectionStateV1::Closed {
            return Ok(());
        }
        self.state = FederationConnectionStateV1::Closed;
        self.transport
            .close()
            .map_err(|_| FederationConnectionRefusalV1::CloseFailed)
    }
}

impl<T> fmt::Debug for FederationConnectionV1<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederationConnectionV1")
            .field("session", &"<redacted>")
            .field("transport", &"<redacted>")
            .field("state", &self.state)
            .finish()
    }
}
