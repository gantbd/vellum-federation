use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, FederationCapabilitiesV1, FederationHelloV1, FederationLimitsV1,
    HomeLocalWorkspaceIdV1, ProfileRangeV1, ReplicaAcknowledgementV1, ServiceIncarnationV1,
    TrustEpochV1, VellumHomeIdV1,
};
use vellum_federation_wire::{
    WireRefusalV1, decode_acknowledgement, decode_event, decode_hello, decode_snapshot,
    encode_acknowledgement, encode_event, encode_hello, encode_snapshot,
};

fn workspace() -> FederatedWorkspaceRefV1 {
    FederatedWorkspaceRefV1 {
        home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
        workspace: HomeLocalWorkspaceIdV1::new([0x22; 16]).unwrap(),
    }
}

fn hello() -> FederationHelloV1 {
    FederationHelloV1 {
        home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
        trust_epoch: TrustEpochV1::new(3).unwrap(),
        incarnation: ServiceIncarnationV1::new([0x33; 16]).unwrap(),
        capabilities: FederationCapabilitiesV1 {
            protocol: ProfileRangeV1::new(1, 2).unwrap(),
            disclosure: ProfileRangeV1::new(1, 2).unwrap(),
            event_schema: ProfileRangeV1::new(3, 4).unwrap(),
            snapshot_schema: ProfileRangeV1::new(5, 6).unwrap(),
            edit_schema: ProfileRangeV1::new(7, 8).unwrap(),
            receipt_schema: ProfileRangeV1::new(9, 10).unwrap(),
            limits: FederationLimitsV1 {
                max_frame_bytes: 1024,
                max_source_bytes: 2048,
                max_documents: 11,
                max_in_flight: 12,
                max_subscriptions: 13,
                max_replay_events: 14,
                max_snapshot_bytes: 4096,
                max_receipts: 15,
            },
        },
    }
}

#[test]
fn hello_fixture_binds_disclosure_and_positive_limits() {
    let hello = hello();
    let encoded = encode_hello(hello);
    assert_eq!(encoded.len(), 106);
    assert_eq!(&encoded[..4], b"VFH1");
    assert_eq!(decode_hello(&encoded), Ok(hello));
    assert_eq!(decode_hello(&encoded[..105]), Err(WireRefusalV1::Truncated));
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(decode_hello(&trailing), Err(WireRefusalV1::LengthMismatch));
    let mut invalid_disclosure = encoded;
    invalid_disclosure[64..66].copy_from_slice(&0_u16.to_be_bytes());
    assert_eq!(
        decode_hello(&invalid_disclosure),
        Err(WireRefusalV1::InvalidProfile)
    );
    let mut zero_limit = encode_hello(hello);
    zero_limit[84..88].fill(0);
    assert_eq!(decode_hello(&zero_limit), Err(WireRefusalV1::InvalidLimits));
}

#[test]
fn event_and_snapshot_fixtures_are_canonical() {
    let event = AuthoritativeWorkspaceEventV1::new(
        workspace(),
        7,
        DisclosureProfileV1::Metadata,
        vec![0xaa, 0xbb],
    )
    .unwrap();
    let encoded = encode_event(&event).unwrap();
    let mut expected = b"VFW1".to_vec();
    expected.push(1);
    expected.extend_from_slice(&[0x11; 32]);
    expected.extend_from_slice(&[0x22; 16]);
    expected.extend_from_slice(&1_u16.to_be_bytes());
    expected.extend_from_slice(&7_u64.to_be_bytes());
    expected.extend_from_slice(&2_u32.to_be_bytes());
    expected.extend_from_slice(&[0xaa, 0xbb]);
    assert_eq!(encoded, expected);
    assert_eq!(decode_event(&encoded, 2), Ok(event));

    let snapshot = AuthoritativeWorkspaceSnapshotV1::new(
        workspace(),
        9,
        DisclosureProfileV1::Source,
        vec![0xcc],
    )
    .unwrap();
    assert_eq!(
        decode_snapshot(&encode_snapshot(&snapshot).unwrap(), 1),
        Ok(snapshot)
    );
}

#[test]
fn acknowledgement_fixture_binds_disclosure() {
    let acknowledgement = ReplicaAcknowledgementV1 {
        workspace: workspace(),
        sequence: 12,
    };
    let frame = encode_acknowledgement(acknowledgement, DisclosureProfileV1::Source);
    assert_eq!(
        decode_acknowledgement(&frame),
        Ok((acknowledgement, DisclosureProfileV1::Source))
    );
}

#[test]
fn decoder_refuses_cross_type_trailing_unknown_and_oversize_frames() {
    let event = AuthoritativeWorkspaceEventV1::new(
        workspace(),
        1,
        DisclosureProfileV1::Metadata,
        vec![1, 2],
    )
    .unwrap();
    let frame = encode_event(&event).unwrap();
    assert_eq!(decode_event(&frame, 1), Err(WireRefusalV1::PayloadTooLarge));
    assert_eq!(
        decode_snapshot(&frame, 2),
        Err(WireRefusalV1::WrongFrameType)
    );
    let mut trailing = frame.clone();
    trailing.push(0);
    assert_eq!(
        decode_event(&trailing, 3),
        Err(WireRefusalV1::LengthMismatch)
    );
    let mut unknown_disclosure = frame;
    unknown_disclosure[53..55].copy_from_slice(&99_u16.to_be_bytes());
    assert_eq!(
        decode_event(&unknown_disclosure, 2),
        Err(WireRefusalV1::InvalidDisclosure)
    );
}
