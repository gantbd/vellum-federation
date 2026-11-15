use vellum_federation_configured_trust::{
    ConfiguredTrustContextV1, ConfiguredTrustV1, PeerProvenanceV1,
};
use vellum_federation_protocol::{
    DisclosureProfileV1, FederationCapabilitiesV1, FederationHelloV1, FederationLimitsV1,
    ProfileRangeV1, ServiceIncarnationV1, TransportExporterV1, TrustEpochV1, VellumHomeIdV1,
    VerifiedHomeV1,
};
use vellum_federation_session::{
    SessionEstablishmentRefusalV1, SessionRevalidationRefusalV1, establish_session_v1,
    revalidate_session_v1,
};

fn hello(marker: u8, epoch: u64, incarnation: u8, disclosure: ProfileRangeV1) -> FederationHelloV1 {
    let profile = ProfileRangeV1::new(1, 2).unwrap();
    FederationHelloV1 {
        home: VellumHomeIdV1::new([marker; 32]).unwrap(),
        trust_epoch: TrustEpochV1::new(epoch).unwrap(),
        incarnation: ServiceIncarnationV1::new([incarnation; 16]).unwrap(),
        capabilities: FederationCapabilitiesV1 {
            protocol: profile,
            disclosure,
            event_schema: profile,
            snapshot_schema: profile,
            edit_schema: profile,
            receipt_schema: profile,
            limits: FederationLimitsV1 {
                max_frame_bytes: 8_192,
                max_source_bytes: 65_536,
                max_documents: 16,
                max_in_flight: 4,
                max_subscriptions: 4,
                max_replay_events: 16,
                max_snapshot_bytes: 262_144,
                max_receipts: 32,
            },
        },
    }
}

fn verified(hello: FederationHelloV1) -> VerifiedHomeV1 {
    VerifiedHomeV1 {
        home: hello.home,
        trust_epoch: hello.trust_epoch,
    }
}

#[test]
fn source_session_binds_trust_incarnations_exporter_profiles_and_disclosure() {
    let source = ProfileRangeV1::new(2, 2).unwrap();
    let local = hello(1, 3, 5, source);
    let peer = hello(2, 4, 6, source);
    let exporter = TransportExporterV1::new([7; 32]).unwrap();
    let context = ConfiguredTrustContextV1::new(
        verified(local),
        verified(peer),
        exporter,
        DisclosureProfileV1::Source,
        PeerProvenanceV1::VerifiedCertificate,
    );
    let session =
        establish_session_v1(&mut ConfiguredTrustV1, &context, local, peer, exporter).unwrap();
    assert_eq!(session.local_incarnation, local.incarnation);
    assert_eq!(session.peer_incarnation, peer.incarnation);
    assert_eq!(session.exporter, exporter);
    assert_eq!(session.negotiated.disclosure, DisclosureProfileV1::Source);
    assert_eq!(
        revalidate_session_v1(&mut ConfiguredTrustV1, &context, session),
        Ok(())
    );

    let disclosure_revoked = ConfiguredTrustContextV1::new(
        verified(local),
        verified(peer),
        exporter,
        DisclosureProfileV1::Metadata,
        PeerProvenanceV1::VerifiedCertificate,
    );
    assert_eq!(
        revalidate_session_v1(&mut ConfiguredTrustV1, &disclosure_revoked, session),
        Err(SessionRevalidationRefusalV1::Trust)
    );
}

#[test]
fn metadata_session_never_upgrades_and_substitution_fails_closed() {
    let metadata = ProfileRangeV1::new(1, 1).unwrap();
    let both = ProfileRangeV1::new(1, 2).unwrap();
    let local = hello(1, 3, 5, both);
    let peer = hello(2, 4, 6, metadata);
    let exporter = TransportExporterV1::new([7; 32]).unwrap();
    let context = ConfiguredTrustContextV1::new(
        verified(local),
        verified(peer),
        exporter,
        DisclosureProfileV1::Metadata,
        PeerProvenanceV1::VerifiedCertificate,
    );
    let session =
        establish_session_v1(&mut ConfiguredTrustV1, &context, local, peer, exporter).unwrap();
    assert_eq!(session.negotiated.disclosure, DisclosureProfileV1::Metadata);

    let mut substituted = peer;
    substituted.home = VellumHomeIdV1::new([8; 32]).unwrap();
    assert_eq!(
        establish_session_v1(
            &mut ConfiguredTrustV1,
            &context,
            local,
            substituted,
            exporter,
        ),
        Err(SessionEstablishmentRefusalV1::Trust)
    );
    assert_eq!(
        establish_session_v1(
            &mut ConfiguredTrustV1,
            &context,
            local,
            peer,
            TransportExporterV1::new([9; 32]).unwrap(),
        ),
        Err(SessionEstablishmentRefusalV1::Trust)
    );
}

#[test]
fn one_home_cannot_federate_with_itself() {
    let metadata = ProfileRangeV1::new(1, 1).unwrap();
    let local = hello(1, 3, 5, metadata);
    let peer = FederationHelloV1 {
        incarnation: ServiceIncarnationV1::new([6; 16]).unwrap(),
        ..local
    };
    let exporter = TransportExporterV1::new([7; 32]).unwrap();
    let context = ConfiguredTrustContextV1::new(
        verified(local),
        verified(peer),
        exporter,
        DisclosureProfileV1::Metadata,
        PeerProvenanceV1::VerifiedCertificate,
    );
    assert!(matches!(
        establish_session_v1(&mut ConfiguredTrustV1, &context, local, peer, exporter,),
        Err(SessionEstablishmentRefusalV1::Session(_))
    ));
}
