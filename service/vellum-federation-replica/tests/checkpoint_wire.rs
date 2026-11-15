use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, HomeLocalWorkspaceIdV1, VellumHomeIdV1,
};
use vellum_federation_replica::checkpoint::{
    CheckpointWireRefusalV1, decode_checkpoint_v1, encode_checkpoint_v1,
};
use vellum_federation_replica::{ReplicaCheckpointV1, ReplicaLimitsV1};

fn limits() -> ReplicaLimitsV1 {
    ReplicaLimitsV1 {
        max_events: 2,
        max_event_bytes: 8,
        max_snapshot_bytes: 8,
        max_retained_bytes: 16,
    }
}

fn checkpoint() -> ReplicaCheckpointV1 {
    let workspace = FederatedWorkspaceRefV1 {
        home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
        workspace: HomeLocalWorkspaceIdV1::new([0x22; 16]).unwrap(),
    };
    ReplicaCheckpointV1 {
        subscriber: VellumHomeIdV1::new([0x33; 32]).unwrap(),
        workspace,
        disclosure: DisclosureProfileV1::Source,
        next_sequence: 4,
        snapshot: Some(
            AuthoritativeWorkspaceSnapshotV1::new(
                workspace,
                2,
                DisclosureProfileV1::Source,
                vec![0xaa, 0xbb],
            )
            .unwrap(),
        ),
        events: vec![
            AuthoritativeWorkspaceEventV1::new(
                workspace,
                3,
                DisclosureProfileV1::Source,
                vec![0xcc],
            )
            .unwrap(),
        ],
    }
}

#[test]
fn replica_checkpoint_is_canonical_and_reencodes_exactly() {
    let checkpoint = checkpoint();
    let bytes = encode_checkpoint_v1(&checkpoint, limits()).unwrap();
    assert_eq!(&bytes[..8], b"VLFRCP01");
    assert_eq!(&bytes[8..10], &1_u16.to_be_bytes());
    assert_eq!(&bytes[10..42], &[0x33; 32]);
    assert_eq!(&bytes[42..74], &[0x11; 32]);
    assert_eq!(&bytes[74..90], &[0x22; 16]);
    assert_eq!(
        &bytes[90..92],
        &(DisclosureProfileV1::Source as u16).to_be_bytes()
    );
    assert_eq!(&bytes[92..100], &4_u64.to_be_bytes());
    assert_eq!(bytes[100], 1);
    assert_eq!(decode_checkpoint_v1(&bytes, limits()), Ok(checkpoint));
    assert_eq!(
        encode_checkpoint_v1(&decode_checkpoint_v1(&bytes, limits()).unwrap(), limits()).unwrap(),
        bytes
    );
}

#[test]
fn checkpoint_decoder_refuses_identity_tag_length_limit_and_sequence_mutants() {
    let valid = encode_checkpoint_v1(&checkpoint(), limits()).unwrap();
    assert_eq!(
        decode_checkpoint_v1(&valid[..104], limits()),
        Err(CheckpointWireRefusalV1::Length)
    );
    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        decode_checkpoint_v1(&trailing, limits()),
        Err(CheckpointWireRefusalV1::Length)
    );
    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        decode_checkpoint_v1(&bad_magic, limits()),
        Err(CheckpointWireRefusalV1::Magic)
    );
    let mut bad_version = valid.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_be_bytes());
    assert_eq!(
        decode_checkpoint_v1(&bad_version, limits()),
        Err(CheckpointWireRefusalV1::Version)
    );
    for range in [10..42, 42..74, 74..90] {
        let mut zero = valid.clone();
        zero[range].fill(0);
        assert_eq!(
            decode_checkpoint_v1(&zero, limits()),
            Err(CheckpointWireRefusalV1::InvalidIdentity)
        );
    }
    let mut bad_disclosure = valid.clone();
    bad_disclosure[90..92].copy_from_slice(&99_u16.to_be_bytes());
    assert_eq!(
        decode_checkpoint_v1(&bad_disclosure, limits()),
        Err(CheckpointWireRefusalV1::InvalidTag)
    );
    let mut bad_snapshot_tag = valid.clone();
    bad_snapshot_tag[100] = 9;
    assert_eq!(
        decode_checkpoint_v1(&bad_snapshot_tag, limits()),
        Err(CheckpointWireRefusalV1::InvalidTag)
    );
    let mut zero_next = valid.clone();
    zero_next[92..100].fill(0);
    assert_eq!(
        decode_checkpoint_v1(&zero_next, limits()),
        Err(CheckpointWireRefusalV1::Malformed)
    );
    assert_eq!(
        decode_checkpoint_v1(
            &valid,
            ReplicaLimitsV1 {
                max_snapshot_bytes: 1,
                ..limits()
            }
        ),
        Err(CheckpointWireRefusalV1::Limit)
    );
}

#[test]
fn encoder_refuses_nonrecoverable_checkpoint_state() {
    let mut invalid = checkpoint();
    invalid.next_sequence = 5;
    assert_eq!(
        encode_checkpoint_v1(&invalid, limits()),
        Err(CheckpointWireRefusalV1::Malformed)
    );
}
