use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, HomeLocalWorkspaceIdV1, VellumHomeIdV1,
};
use vellum_federation_replica::{
    ReplicaApplyV1, ReplicaJournalV1, ReplicaLimitsV1, ReplicaRefusalV1,
};

fn home(marker: u8) -> VellumHomeIdV1 {
    VellumHomeIdV1::new([marker; 32]).unwrap()
}

fn workspace(home_marker: u8, workspace_marker: u8) -> FederatedWorkspaceRefV1 {
    FederatedWorkspaceRefV1 {
        home: home(home_marker),
        workspace: HomeLocalWorkspaceIdV1::new([workspace_marker; 16]).unwrap(),
    }
}

fn limits() -> ReplicaLimitsV1 {
    ReplicaLimitsV1 {
        max_events: 4,
        max_event_bytes: 64,
        max_snapshot_bytes: 128,
        max_retained_bytes: 192,
    }
}

#[test]
fn direct_home_events_apply_replay_gap_and_divergence_exactly() {
    let scope = workspace(1, 7);
    let mut replica =
        ReplicaJournalV1::new(home(2), scope, DisclosureProfileV1::Metadata, limits()).unwrap();
    let first = AuthoritativeWorkspaceEventV1::new(
        scope,
        1,
        DisclosureProfileV1::Metadata,
        b"document-revision-advanced".to_vec(),
    )
    .unwrap();
    assert_eq!(
        replica.apply(home(1), first.clone()),
        Ok(ReplicaApplyV1::Applied)
    );
    assert_eq!(replica.apply(home(1), first), Ok(ReplicaApplyV1::Replay));
    assert_eq!(
        replica.apply(
            home(1),
            AuthoritativeWorkspaceEventV1::new(
                scope,
                1,
                DisclosureProfileV1::Metadata,
                b"changed".to_vec()
            )
            .unwrap()
        ),
        Err(ReplicaRefusalV1::Divergence)
    );
    assert_eq!(
        replica.apply(
            home(1),
            AuthoritativeWorkspaceEventV1::new(
                scope,
                3,
                DisclosureProfileV1::Metadata,
                b"gap".to_vec()
            )
            .unwrap()
        ),
        Err(ReplicaRefusalV1::Gap {
            expected: 2,
            observed: 3
        })
    );
}

#[test]
fn metadata_replica_refuses_source_bytes_and_imported_export() {
    let scope = workspace(1, 7);
    let mut replica =
        ReplicaJournalV1::new(home(2), scope, DisclosureProfileV1::Metadata, limits()).unwrap();
    let source_event = AuthoritativeWorkspaceEventV1::new(
        scope,
        1,
        DisclosureProfileV1::Source,
        b"secret source".to_vec(),
    )
    .unwrap();
    assert_eq!(
        replica.apply(home(1), source_event),
        Err(ReplicaRefusalV1::DisclosureMismatch)
    );
    assert_eq!(
        replica.apply(
            home(3),
            AuthoritativeWorkspaceEventV1::new(
                scope,
                1,
                DisclosureProfileV1::Metadata,
                b"metadata".to_vec()
            )
            .unwrap()
        ),
        Err(ReplicaRefusalV1::NonHomeSource)
    );
    assert!(!replica.can_export_authoritatively());
}

#[test]
fn source_snapshot_repairs_gap_and_checkpoint_preserves_disclosure() {
    let scope = workspace(1, 7);
    let mut replica =
        ReplicaJournalV1::new(home(2), scope, DisclosureProfileV1::Source, limits()).unwrap();
    assert_eq!(
        replica.apply_snapshot(
            home(1),
            AuthoritativeWorkspaceSnapshotV1::new(
                scope,
                4,
                DisclosureProfileV1::Source,
                b"authorized source snapshot".to_vec()
            )
            .unwrap()
        ),
        Ok(ReplicaApplyV1::SnapshotApplied)
    );
    let fifth = AuthoritativeWorkspaceEventV1::new(
        scope,
        5,
        DisclosureProfileV1::Source,
        b"source replacement".to_vec(),
    )
    .unwrap();
    assert_eq!(
        replica.apply(home(1), fifth.clone()),
        Ok(ReplicaApplyV1::Applied)
    );
    let checkpoint = replica.checkpoint();
    assert_eq!(checkpoint.disclosure, DisclosureProfileV1::Source);
    let mut recovered = ReplicaJournalV1::recover(checkpoint, limits()).unwrap();
    assert_eq!(recovered.acknowledgement(), 5);
    assert_eq!(recovered.apply(home(1), fifth), Ok(ReplicaApplyV1::Replay));
}
