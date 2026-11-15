use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, HomeLocalWorkspaceIdV1, ReplicaAcknowledgementV1, VellumHomeIdV1,
};
use vellum_federation_wire::{
    WireRefusalV1, decode_acknowledgement, decode_event, decode_snapshot, encode_acknowledgement,
    encode_event, encode_snapshot,
};

fn workspace() -> FederatedWorkspaceRefV1 {
    FederatedWorkspaceRefV1 {
        home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
        workspace: HomeLocalWorkspaceIdV1::new([0x22; 16]).unwrap(),
    }
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
