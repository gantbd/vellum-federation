use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    FederatedWorkspaceRefV1, HomeLocalWorkspaceIdV1, VellumHomeIdV1,
};
use vellum_federation_replica::{ReplicaApplyV1, ReplicaJournalV1, ReplicaLimitsV1};
use vellum_federation_replica_durable::{
    DurableReplicaOpenRefusalV1, DurableReplicaPublicationV1, DurableReplicaV1,
};
use vellum_federation_replica_store_port::{
    ReplicaStoreLoadV1, ReplicaStoreOfferV1, ReplicaStorePortV1, ReplicaStoreRecordV1,
    ReplicaStoreScopeV1,
};

#[derive(Clone, Copy)]
enum NextOffer {
    Normal,
    Refuse,
    UnknownAfterCommit,
    InvalidAfterCommit,
}

struct MemoryStore {
    record: Option<ReplicaStoreRecordV1>,
    next: NextOffer,
    offers: usize,
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            record: None,
            next: NextOffer::Normal,
            offers: 0,
        }
    }
}

impl ReplicaStorePortV1 for MemoryStore {
    fn load(&mut self, scope: ReplicaStoreScopeV1) -> ReplicaStoreLoadV1 {
        self.record
            .as_ref()
            .map_or(ReplicaStoreLoadV1::Missing, |record| {
                if record.scope == scope {
                    ReplicaStoreLoadV1::Present(record.clone())
                } else {
                    ReplicaStoreLoadV1::Missing
                }
            })
    }

    fn compare_exchange(
        &mut self,
        expected_revision: Option<u64>,
        proposed: &ReplicaStoreRecordV1,
    ) -> ReplicaStoreOfferV1 {
        self.offers += 1;
        let actual = self.record.as_ref().map(|record| record.revision);
        if actual != expected_revision {
            return ReplicaStoreOfferV1::NotCommitted;
        }
        let next = self.next;
        self.next = NextOffer::Normal;
        match next {
            NextOffer::Normal => {
                self.record = Some(proposed.clone());
                ReplicaStoreOfferV1::Committed(proposed.clone())
            }
            NextOffer::Refuse => ReplicaStoreOfferV1::NotCommitted,
            NextOffer::UnknownAfterCommit => {
                self.record = Some(proposed.clone());
                ReplicaStoreOfferV1::OutcomeUnknown
            }
            NextOffer::InvalidAfterCommit => {
                self.record = Some(proposed.clone());
                let invalid = ReplicaStoreRecordV1::new(
                    proposed.scope,
                    proposed.revision + 1,
                    proposed.checkpoint().to_vec(),
                    proposed.checkpoint().len(),
                )
                .unwrap();
                ReplicaStoreOfferV1::Committed(invalid)
            }
        }
    }
}

fn home(marker: u8) -> VellumHomeIdV1 {
    VellumHomeIdV1::new([marker; 32]).unwrap()
}

fn workspace() -> FederatedWorkspaceRefV1 {
    FederatedWorkspaceRefV1 {
        home: home(1),
        workspace: HomeLocalWorkspaceIdV1::new([7; 16]).unwrap(),
    }
}

fn limits() -> ReplicaLimitsV1 {
    ReplicaLimitsV1 {
        max_events: 8,
        max_event_bytes: 64,
        max_snapshot_bytes: 128,
        max_retained_bytes: 256,
    }
}

fn event(sequence: u64) -> AuthoritativeWorkspaceEventV1 {
    AuthoritativeWorkspaceEventV1::new(
        workspace(),
        sequence,
        DisclosureProfileV1::Metadata,
        vec![sequence as u8; 8],
    )
    .unwrap()
}

fn initialized(store: &mut MemoryStore) -> DurableReplicaV1 {
    DurableReplicaV1::initialize(
        ReplicaJournalV1::new(
            home(2),
            workspace(),
            DisclosureProfileV1::Metadata,
            limits(),
        )
        .unwrap(),
        limits(),
        ReplicaStoreScopeV1::new([9; 32]).unwrap(),
        1_024,
        store,
    )
    .unwrap()
}

#[test]
fn commit_refusal_replay_and_recovery_preserve_exact_disclosure_bound_progress() {
    let mut store = MemoryStore::new();
    let mut durable = initialized(&mut store);
    assert_eq!(
        durable.apply_event(home(1), event(1), &mut store),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::Applied)
    );
    let offers = store.offers;
    assert_eq!(
        durable.apply_event(home(1), event(1), &mut store),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::Replay)
    );
    assert_eq!(store.offers, offers);

    store.next = NextOffer::Refuse;
    assert_eq!(
        durable.apply_event(home(1), event(2), &mut store),
        DurableReplicaPublicationV1::StoreRefused
    );
    assert_eq!(durable.acknowledgement(), 1);
    assert_eq!(
        durable.apply_event(home(1), event(2), &mut store),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::Applied)
    );
    drop(durable);

    let mut recovered = DurableReplicaV1::recover(
        limits(),
        ReplicaStoreScopeV1::new([9; 32]).unwrap(),
        1_024,
        &mut store,
    )
    .unwrap();
    assert_eq!(recovered.acknowledgement(), 2);
    assert_eq!(recovered.store_revision(), 2);
    assert!(!recovered.can_export_authoritatively());
    assert_eq!(
        recovered.apply_event(
            home(1),
            AuthoritativeWorkspaceEventV1::new(
                workspace(),
                3,
                DisclosureProfileV1::Source,
                b"source".to_vec(),
            )
            .unwrap(),
            &mut store
        ),
        DurableReplicaPublicationV1::Refused(
            vellum_federation_replica::ReplicaRefusalV1::DisclosureMismatch
        )
    );
}

#[test]
fn unknown_and_invalid_receipts_fence_until_fresh_recovery() {
    let mut initialization_store = MemoryStore::new();
    initialization_store.next = NextOffer::InvalidAfterCommit;
    assert!(matches!(
        DurableReplicaV1::initialize(
            ReplicaJournalV1::new(
                home(2),
                workspace(),
                DisclosureProfileV1::Metadata,
                limits(),
            )
            .unwrap(),
            limits(),
            ReplicaStoreScopeV1::new([9; 32]).unwrap(),
            1_024,
            &mut initialization_store,
        ),
        Err(DurableReplicaOpenRefusalV1::OutcomeUnknown)
    ));
    assert_eq!(
        DurableReplicaV1::recover(
            limits(),
            ReplicaStoreScopeV1::new([9; 32]).unwrap(),
            1_024,
            &mut initialization_store,
        )
        .unwrap()
        .acknowledgement(),
        0
    );

    let mut store = MemoryStore::new();
    let mut durable = initialized(&mut store);
    store.next = NextOffer::UnknownAfterCommit;
    assert_eq!(
        durable.apply_event(home(1), event(1), &mut store),
        DurableReplicaPublicationV1::OutcomeUnknown
    );
    assert!(durable.is_fenced());
    assert_eq!(
        durable.apply_event(home(1), event(1), &mut store),
        DurableReplicaPublicationV1::Fenced
    );
    drop(durable);
    let recovered = DurableReplicaV1::recover(
        limits(),
        ReplicaStoreScopeV1::new([9; 32]).unwrap(),
        1_024,
        &mut store,
    )
    .unwrap();
    assert_eq!(recovered.acknowledgement(), 1);

    let mut second_store = MemoryStore::new();
    let mut second = initialized(&mut second_store);
    second_store.next = NextOffer::InvalidAfterCommit;
    assert_eq!(
        second.apply_event(home(1), event(1), &mut second_store),
        DurableReplicaPublicationV1::OutcomeUnknown
    );
    assert!(second.is_fenced());
}

#[test]
fn snapshot_repair_is_commit_gated_disclosure_bound_and_recoverable() {
    let mut store = MemoryStore::new();
    let mut durable = initialized(&mut store);
    let snapshot = AuthoritativeWorkspaceSnapshotV1::new(
        workspace(),
        4,
        DisclosureProfileV1::Metadata,
        b"metadata snapshot".to_vec(),
    )
    .unwrap();
    store.next = NextOffer::Refuse;
    assert_eq!(
        durable.apply_snapshot(home(1), snapshot.clone(), &mut store),
        DurableReplicaPublicationV1::StoreRefused
    );
    assert_eq!(durable.acknowledgement(), 0);
    assert_eq!(
        durable.apply_snapshot(home(1), snapshot.clone(), &mut store),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::SnapshotApplied)
    );
    let offers = store.offers;
    assert_eq!(
        durable.apply_snapshot(home(1), snapshot, &mut store),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::Replay)
    );
    assert_eq!(store.offers, offers);
    drop(durable);
    assert_eq!(
        DurableReplicaV1::recover(
            limits(),
            ReplicaStoreScopeV1::new([9; 32]).unwrap(),
            1_024,
            &mut store,
        )
        .unwrap()
        .acknowledgement(),
        4
    );
}
