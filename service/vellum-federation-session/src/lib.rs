//! Trust and disclosure-gated construction of Vellum federation sessions.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{
    DisclosureProfileV1, FederationHelloV1, FederationSessionV1, SessionRefusalV1,
    TransportExporterV1, VerifiedHomeV1, bind_session_v1, negotiate_v1,
};
use vellum_federation_trust_port::{
    FederationTrustPortV1, FederationTrustRequestV1, FederationTrustRoleV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionEstablishmentRefusalV1 {
    Negotiation,
    Trust,
    Session(SessionRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRevalidationRefusalV1 {
    Trust,
    IdentityChanged,
}

pub fn establish_session_v1<T: FederationTrustPortV1>(
    trust: &mut T,
    context: &T::Context,
    local_hello: FederationHelloV1,
    peer_hello: FederationHelloV1,
    exporter: TransportExporterV1,
) -> Result<FederationSessionV1, SessionEstablishmentRefusalV1> {
    let disclosure = negotiate_v1(local_hello.capabilities, peer_hello.capabilities)
        .map_err(|_| SessionEstablishmentRefusalV1::Negotiation)?
        .disclosure;
    let local = verify(
        trust,
        context,
        FederationTrustRoleV1::Local,
        local_hello,
        exporter,
        disclosure,
    )?;
    let peer = verify(
        trust,
        context,
        FederationTrustRoleV1::Peer,
        peer_hello,
        exporter,
        disclosure,
    )?;
    bind_session_v1(local, peer, local_hello, peer_hello, exporter)
        .map_err(SessionEstablishmentRefusalV1::Session)
}

pub fn revalidate_session_v1<T: FederationTrustPortV1>(
    trust: &mut T,
    context: &T::Context,
    session: FederationSessionV1,
) -> Result<(), SessionRevalidationRefusalV1> {
    let local = reverify(trust, context, FederationTrustRoleV1::Local, session)?;
    let peer = reverify(trust, context, FederationTrustRoleV1::Peer, session)?;
    if local != session.local || peer != session.peer {
        return Err(SessionRevalidationRefusalV1::IdentityChanged);
    }
    Ok(())
}

fn verify<T: FederationTrustPortV1>(
    trust: &mut T,
    context: &T::Context,
    role: FederationTrustRoleV1,
    hello: FederationHelloV1,
    exporter: TransportExporterV1,
    disclosure: DisclosureProfileV1,
) -> Result<VerifiedHomeV1, SessionEstablishmentRefusalV1> {
    trust
        .verify(
            FederationTrustRequestV1 {
                role,
                claimed: VerifiedHomeV1 {
                    home: hello.home,
                    trust_epoch: hello.trust_epoch,
                },
                exporter,
                disclosure,
            },
            context,
        )
        .map_err(|_| SessionEstablishmentRefusalV1::Trust)
}

fn reverify<T: FederationTrustPortV1>(
    trust: &mut T,
    context: &T::Context,
    role: FederationTrustRoleV1,
    session: FederationSessionV1,
) -> Result<VerifiedHomeV1, SessionRevalidationRefusalV1> {
    let claimed = match role {
        FederationTrustRoleV1::Local => session.local,
        FederationTrustRoleV1::Peer => session.peer,
    };
    trust
        .verify(
            FederationTrustRequestV1 {
                role,
                claimed,
                exporter: session.exporter,
                disclosure: session.negotiated.disclosure,
            },
            context,
        )
        .map_err(|_| SessionRevalidationRefusalV1::Trust)
}
