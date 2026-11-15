use vellum_federation_configured_trust::{
    ConfiguredTrustContextV1, ConfiguredTrustRefusalV1, ConfiguredTrustV1, PeerProvenanceV1,
};
use vellum_federation_protocol::{
    DisclosureProfileV1, TransportExporterV1, TrustEpochV1, VellumHomeIdV1, VerifiedHomeV1,
};
use vellum_federation_trust_port::{
    FederationTrustPortV1, FederationTrustRequestV1, FederationTrustRoleV1,
};

fn verified(marker: u8, epoch: u64) -> VerifiedHomeV1 {
    VerifiedHomeV1 {
        home: VellumHomeIdV1::new([marker; 32]).unwrap(),
        trust_epoch: TrustEpochV1::new(epoch).unwrap(),
    }
}

#[test]
fn certificate_exporter_identity_and_disclosure_are_all_required() {
    let local = verified(1, 3);
    let peer = verified(2, 4);
    let exporter = TransportExporterV1::new([5; 32]).unwrap();
    let source_context = ConfiguredTrustContextV1::new(
        local,
        peer,
        exporter,
        DisclosureProfileV1::Source,
        PeerProvenanceV1::VerifiedCertificate,
    );
    let request = FederationTrustRequestV1 {
        role: FederationTrustRoleV1::Peer,
        claimed: peer,
        exporter,
        disclosure: DisclosureProfileV1::Source,
    };
    let mut trust = ConfiguredTrustV1;
    assert_eq!(trust.verify(request, &source_context), Ok(peer));

    let metadata_context = ConfiguredTrustContextV1::new(
        local,
        peer,
        exporter,
        DisclosureProfileV1::Metadata,
        PeerProvenanceV1::VerifiedCertificate,
    );
    assert_eq!(
        trust.verify(request, &metadata_context),
        Err(ConfiguredTrustRefusalV1::DisclosureDenied)
    );
    let encrypted_only = ConfiguredTrustContextV1::new(
        local,
        peer,
        exporter,
        DisclosureProfileV1::Source,
        PeerProvenanceV1::EncryptedChannel,
    );
    assert_eq!(
        trust.verify(request, &encrypted_only),
        Err(ConfiguredTrustRefusalV1::UnauthenticatedPeer)
    );
    assert_eq!(
        trust.verify(
            FederationTrustRequestV1 {
                exporter: TransportExporterV1::new([6; 32]).unwrap(),
                ..request
            },
            &source_context,
        ),
        Err(ConfiguredTrustRefusalV1::ExporterMismatch)
    );
    assert_eq!(
        trust.verify(
            FederationTrustRequestV1 {
                claimed: verified(7, 4),
                ..request
            },
            &source_context,
        ),
        Err(ConfiguredTrustRefusalV1::IdentityMismatch)
    );
}

#[test]
fn diagnostics_redact_home_epoch_and_exporter() {
    let local = verified(1, 3);
    let peer = verified(2, 4);
    let exporter = TransportExporterV1::new([5; 32]).unwrap();
    let context = ConfiguredTrustContextV1::new(
        local,
        peer,
        exporter,
        DisclosureProfileV1::Metadata,
        PeerProvenanceV1::VerifiedCertificate,
    );
    let diagnostics = format!(
        "{context:?} {:?}",
        FederationTrustRequestV1 {
            role: FederationTrustRoleV1::Peer,
            claimed: peer,
            exporter,
            disclosure: DisclosureProfileV1::Metadata,
        }
    );
    assert!(diagnostics.contains("<redacted>"));
    assert!(!diagnostics.contains("trust_epoch: 4"));
    assert!(!diagnostics.contains("[5, 5"));
}
