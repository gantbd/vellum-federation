use vellum_federation_connection::{
    FederationConnectionRefusalV1, FederationConnectionStateV1, FederationConnectionV1,
};
use vellum_federation_protocol::{
    DisclosureProfileV1, FederationLimitsV1, FederationSessionV1, NegotiatedFederationV1,
    ServiceIncarnationV1, TransportExporterV1, TrustEpochV1, VellumHomeIdV1, VerifiedHomeV1,
};
use vellum_federation_transport_port::FederationTransportLifecyclePortV1;
use vellum_federation_trust_port::{FederationTrustPortV1, FederationTrustRequestV1};

#[derive(Default)]
struct Transport {
    closes: usize,
    fail: bool,
}

impl FederationTransportLifecyclePortV1 for Transport {
    type Error = ();

    fn close(&mut self) -> Result<(), Self::Error> {
        self.closes += 1;
        if self.fail { Err(()) } else { Ok(()) }
    }
}

#[derive(Clone, Copy)]
struct TrustContext {
    allowed: bool,
    maximum_disclosure: DisclosureProfileV1,
}

struct Trust;

impl FederationTrustPortV1 for Trust {
    type Context = TrustContext;
    type Error = ();

    fn verify(
        &mut self,
        request: FederationTrustRequestV1,
        context: &Self::Context,
    ) -> Result<VerifiedHomeV1, Self::Error> {
        if context.allowed && request.disclosure <= context.maximum_disclosure {
            Ok(request.claimed)
        } else {
            Err(())
        }
    }
}

fn verified(marker: u8, epoch: u64) -> VerifiedHomeV1 {
    VerifiedHomeV1 {
        home: VellumHomeIdV1::new([marker; 32]).unwrap(),
        trust_epoch: TrustEpochV1::new(epoch).unwrap(),
    }
}

fn session() -> FederationSessionV1 {
    FederationSessionV1 {
        local: verified(1, 3),
        peer: verified(2, 4),
        local_incarnation: ServiceIncarnationV1::new([5; 16]).unwrap(),
        peer_incarnation: ServiceIncarnationV1::new([6; 16]).unwrap(),
        exporter: TransportExporterV1::new([7; 32]).unwrap(),
        negotiated: NegotiatedFederationV1 {
            protocol: 1,
            disclosure: DisclosureProfileV1::Source,
            event_schema: 1,
            snapshot_schema: 1,
            edit_schema: 1,
            receipt_schema: 1,
            limits: FederationLimitsV1 {
                max_frame_bytes: 1_024,
                max_source_bytes: 2_048,
                max_documents: 4,
                max_in_flight: 4,
                max_subscriptions: 4,
                max_replay_events: 8,
                max_snapshot_bytes: 4_096,
                max_receipts: 8,
            },
        },
    }
}

fn source_allowed() -> TrustContext {
    TrustContext {
        allowed: true,
        maximum_disclosure: DisclosureProfileV1::Source,
    }
}

#[test]
fn trust_revocation_closes_exactly_once_and_hides_the_session() {
    let mut connection = FederationConnectionV1::new(session(), Transport::default());
    assert_eq!(connection.revalidate(&mut Trust, &source_allowed()), Ok(()));
    assert_eq!(
        connection.revalidate(
            &mut Trust,
            &TrustContext {
                allowed: false,
                maximum_disclosure: DisclosureProfileV1::Source,
            },
        ),
        Err(FederationConnectionRefusalV1::TrustOrDisclosureRevoked)
    );
    assert_eq!(connection.state(), FederationConnectionStateV1::Closed);
    assert!(connection.session().is_none());
    assert_eq!(connection.transport().closes, 1);
    assert_eq!(connection.close(), Ok(()));
    assert_eq!(connection.transport().closes, 1);
}

#[test]
fn disclosure_downgrade_closes_exactly_once() {
    let mut connection = FederationConnectionV1::new(session(), Transport::default());
    assert_eq!(
        connection.revalidate(
            &mut Trust,
            &TrustContext {
                allowed: true,
                maximum_disclosure: DisclosureProfileV1::Metadata,
            },
        ),
        Err(FederationConnectionRefusalV1::TrustOrDisclosureRevoked)
    );
    assert_eq!(connection.state(), FederationConnectionStateV1::Closed);
    assert_eq!(connection.transport().closes, 1);
    assert_eq!(
        connection.revalidate(&mut Trust, &source_allowed()),
        Err(FederationConnectionRefusalV1::Closed)
    );
    assert_eq!(connection.transport().closes, 1);
}

#[test]
fn close_failure_is_visible_but_never_reactivates_the_connection() {
    let mut connection = FederationConnectionV1::new(
        session(),
        Transport {
            closes: 0,
            fail: true,
        },
    );
    assert_eq!(
        connection.revalidate(
            &mut Trust,
            &TrustContext {
                allowed: false,
                maximum_disclosure: DisclosureProfileV1::Source,
            },
        ),
        Err(FederationConnectionRefusalV1::TrustOrDisclosureRevokedCloseFailed)
    );
    assert_eq!(connection.state(), FederationConnectionStateV1::Closed);
    assert!(connection.session().is_none());
    assert_eq!(connection.transport().closes, 1);
    assert_eq!(connection.close(), Ok(()));
    assert_eq!(connection.transport().closes, 1);
    assert!(!format!("{connection:?}").contains("VellumHomeIdV1"));
}
