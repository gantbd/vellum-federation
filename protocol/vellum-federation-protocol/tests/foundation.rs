use vellum_federation_protocol::{
    DisclosureProfileV1, FederatedDocumentRefV1, FederatedWorkspaceRefV1, FederationCapabilitiesV1,
    FederationLimitsV1, HomeLocalDocumentIdV1, HomeLocalWorkspaceIdV1, NegotiationRefusalV1,
    ProfileRangeV1, SourceRevisionV1, VellumHomeIdV1, negotiate_v1,
};

fn limits(marker: u32) -> FederationLimitsV1 {
    FederationLimitsV1 {
        max_frame_bytes: marker * 1_000,
        max_source_bytes: marker * 2_000,
        max_documents: marker as u16,
        max_in_flight: marker as u16 + 1,
        max_subscriptions: marker as u16 + 2,
        max_replay_events: marker as u16 + 3,
        max_snapshot_bytes: marker * 4_000,
        max_receipts: marker as u16 + 4,
    }
}

fn capabilities(disclosure: ProfileRangeV1, marker: u32) -> FederationCapabilitiesV1 {
    let profile = ProfileRangeV1::new(1, 2).unwrap();
    FederationCapabilitiesV1 {
        protocol: profile,
        disclosure,
        event_schema: profile,
        snapshot_schema: profile,
        edit_schema: profile,
        receipt_schema: profile,
        limits: limits(marker),
    }
}

#[test]
fn workspace_document_and_revision_identity_are_scoped_and_nonzero() {
    assert!(VellumHomeIdV1::new([0; 32]).is_none());
    assert!(HomeLocalWorkspaceIdV1::new([0; 16]).is_none());
    assert!(HomeLocalDocumentIdV1::new([0; 16]).is_none());
    assert!(SourceRevisionV1::new(0).is_none());

    let workspace = HomeLocalWorkspaceIdV1::new([2; 16]).unwrap();
    let document = HomeLocalDocumentIdV1::new([3; 16]).unwrap();
    let first = FederatedDocumentRefV1 {
        workspace: FederatedWorkspaceRefV1 {
            home: VellumHomeIdV1::new([1; 32]).unwrap(),
            workspace,
        },
        document,
    };
    let second = FederatedDocumentRefV1 {
        workspace: FederatedWorkspaceRefV1 {
            home: VellumHomeIdV1::new([4; 32]).unwrap(),
            workspace,
        },
        document,
    };
    assert_ne!(first, second);
    assert_eq!(SourceRevisionV1::new(5).unwrap().get(), 5);
    assert!(format!("{first:?}").contains("<redacted>"));
}

#[test]
fn disclosure_is_exactly_negotiated_and_limits_take_the_bounded_minimum() {
    let metadata = ProfileRangeV1::new(1, 1).unwrap();
    let both = ProfileRangeV1::new(1, 2).unwrap();
    let selected = negotiate_v1(capabilities(both, 8), capabilities(metadata, 5)).unwrap();
    assert_eq!(selected.disclosure, DisclosureProfileV1::Metadata);
    assert_eq!(selected.protocol, 2);
    assert_eq!(selected.limits, limits(5));

    let source = ProfileRangeV1::new(2, 2).unwrap();
    assert_eq!(
        negotiate_v1(capabilities(source, 5), capabilities(metadata, 5)),
        Err(NegotiationRefusalV1::Disclosure)
    );
}

#[test]
fn unknown_disclosure_and_zero_limits_fail_closed() {
    let unknown = ProfileRangeV1::new(3, 3).unwrap();
    assert_eq!(
        negotiate_v1(capabilities(unknown, 1), capabilities(unknown, 1)),
        Err(NegotiationRefusalV1::Disclosure)
    );
    let mut invalid = capabilities(ProfileRangeV1::new(1, 1).unwrap(), 1);
    invalid.limits.max_source_bytes = 0;
    assert_eq!(
        negotiate_v1(invalid, capabilities(ProfileRangeV1::new(1, 1).unwrap(), 1)),
        Err(NegotiationRefusalV1::InvalidLimits)
    );
}
