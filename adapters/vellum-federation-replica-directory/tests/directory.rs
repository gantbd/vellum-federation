use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, DisclosureProfileV1, FederatedWorkspaceRefV1,
    HomeLocalWorkspaceIdV1, VellumHomeIdV1,
};
use vellum_federation_replica::{ReplicaApplyV1, ReplicaJournalV1, ReplicaLimitsV1};
use vellum_federation_replica_directory::{DirectoryReplicaOpenRefusalV1, DirectoryReplicaStoreV1};
use vellum_federation_replica_durable::{DurableReplicaPublicationV1, DurableReplicaV1};
use vellum_federation_replica_store_port::{
    ReplicaStoreLoadV1, ReplicaStoreOfferV1, ReplicaStorePortV1, ReplicaStoreRecordV1,
    ReplicaStoreScopeV1,
};

const MAX_CHECKPOINT_BYTES: usize = 4096;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "vellum-federation-replica-{}-{nonce}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn test_scope(byte: u8) -> ReplicaStoreScopeV1 {
    ReplicaStoreScopeV1::new([byte; 32]).unwrap()
}

fn record(scope: ReplicaStoreScopeV1, revision: u64, byte: u8) -> ReplicaStoreRecordV1 {
    ReplicaStoreRecordV1::new(scope, revision, vec![byte; 128], MAX_CHECKPOINT_BYTES).unwrap()
}

fn home(byte: u8) -> VellumHomeIdV1 {
    VellumHomeIdV1::new([byte; 32]).unwrap()
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

#[test]
fn compare_exchange_is_private_atomic_and_recoverable() {
    let directory = TestDirectory::new();
    let scope = test_scope(7);
    let mut store =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    assert_eq!(store.load(scope), ReplicaStoreLoadV1::Missing);

    let initial = record(scope, 0, 1);
    assert_eq!(
        store.compare_exchange(None, &initial),
        ReplicaStoreOfferV1::Committed(initial.clone())
    );
    assert_eq!(
        store.compare_exchange(None, &record(scope, 0, 2)),
        ReplicaStoreOfferV1::NotCommitted
    );
    assert_eq!(store.load(test_scope(8)), ReplicaStoreLoadV1::Unavailable);
    assert!(!format!("{store:?}").contains(directory.path().to_str().unwrap()));
    assert!(!format!("{store:?}").contains("1, 1"));
    drop(store);

    let mut reopened =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    assert_eq!(
        reopened.load(scope),
        ReplicaStoreLoadV1::Present(initial.clone())
    );
    let next = record(scope, 1, 3);
    assert_eq!(
        reopened.compare_exchange(Some(0), &next),
        ReplicaStoreOfferV1::Committed(next.clone())
    );
    drop(reopened);

    let mut recovered =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    assert_eq!(recovered.load(scope), ReplicaStoreLoadV1::Present(next));
    assert_eq!(
        fs::metadata(directory.path()).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(directory.path().join("replica.head"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn corruption_residue_and_unsafe_modes_fail_closed() {
    let directory = TestDirectory::new();
    let scope = test_scope(9);
    let mut store =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    let initial = record(scope, 0, 4);
    assert!(matches!(
        store.compare_exchange(None, &initial),
        ReplicaStoreOfferV1::Committed(_)
    ));
    drop(store);

    let head = directory.path().join("replica.head");
    let canonical = fs::read(&head).unwrap();
    for bit in 0..canonical.len() * 8 {
        let mut mutated = canonical.clone();
        mutated[bit / 8] ^= 1 << (bit % 8);
        fs::write(&head, &mutated).unwrap();
        assert!(matches!(
            DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES),
            Err(DirectoryReplicaOpenRefusalV1::Unavailable)
        ));
    }
    fs::write(&head, canonical).unwrap();

    let mut pending = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.path().join("replica.pending"))
        .unwrap();
    pending.write_all(b"interrupted").unwrap();
    pending.sync_all().unwrap();
    assert!(matches!(
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES),
        Err(DirectoryReplicaOpenRefusalV1::PendingPublication)
    ));
    fs::remove_file(directory.path().join("replica.pending")).unwrap();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.path().join("replica.lock"))
        .unwrap();
    assert!(matches!(
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES),
        Err(DirectoryReplicaOpenRefusalV1::PendingPublication)
    ));
    fs::remove_file(directory.path().join("replica.lock")).unwrap();
    symlink("missing-target", directory.path().join("replica.pending")).unwrap();
    assert!(matches!(
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES),
        Err(DirectoryReplicaOpenRefusalV1::PendingPublication)
    ));
    fs::remove_file(directory.path().join("replica.pending")).unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES),
        Err(DirectoryReplicaOpenRefusalV1::UnsafeDirectory)
    ));
}

#[test]
fn concurrent_compare_exchange_has_one_exact_winner() {
    let directory = TestDirectory::new();
    let scope = test_scope(12);
    let mut initial_store =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    assert!(matches!(
        initial_store.compare_exchange(None, &record(scope, 0, 1)),
        ReplicaStoreOfferV1::Committed(_)
    ));
    drop(initial_store);

    let first =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    let second =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first_record = record(scope, 1, 2);
    let first_thread = std::thread::spawn(move || {
        let mut store = first;
        first_barrier.wait();
        store.compare_exchange(Some(0), &first_record)
    });
    let second_barrier = Arc::clone(&barrier);
    let second_record = record(scope, 1, 3);
    let second_thread = std::thread::spawn(move || {
        let mut store = second;
        second_barrier.wait();
        store.compare_exchange(Some(0), &second_record)
    });
    barrier.wait();
    let outcomes = [first_thread.join().unwrap(), second_thread.join().unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ReplicaStoreOfferV1::Committed(_)))
            .count(),
        1
    );
    assert!(outcomes.iter().any(|outcome| matches!(
        outcome,
        ReplicaStoreOfferV1::NotCommitted | ReplicaStoreOfferV1::OutcomeUnknown
    )));

    let mut reopened =
        DirectoryReplicaStoreV1::open(directory.path(), scope, MAX_CHECKPOINT_BYTES).unwrap();
    let ReplicaStoreLoadV1::Present(stored) = reopened.load(scope) else {
        panic!("one committed record expected");
    };
    assert_eq!(stored.revision, 1);
    assert!(stored.checkpoint() == [2; 128] || stored.checkpoint() == [3; 128]);
}

#[test]
fn durable_disclosure_bound_replica_survives_physical_owner_replacement() {
    let directory = TestDirectory::new();
    let store_scope = test_scope(11);
    let mut store =
        DirectoryReplicaStoreV1::open(directory.path(), store_scope, MAX_CHECKPOINT_BYTES).unwrap();
    let mut durable = DurableReplicaV1::initialize(
        ReplicaJournalV1::new(
            home(2),
            workspace(),
            DisclosureProfileV1::Metadata,
            limits(),
        )
        .unwrap(),
        limits(),
        store_scope,
        MAX_CHECKPOINT_BYTES,
        &mut store,
    )
    .unwrap();
    let event = AuthoritativeWorkspaceEventV1::new(
        workspace(),
        1,
        DisclosureProfileV1::Metadata,
        vec![5; 8],
    )
    .unwrap();
    assert_eq!(
        durable.apply_event(home(1), event.clone(), &mut store),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::Applied)
    );
    drop(durable);
    drop(store);

    let mut reopened =
        DirectoryReplicaStoreV1::open(directory.path(), store_scope, MAX_CHECKPOINT_BYTES).unwrap();
    let mut recovered =
        DurableReplicaV1::recover(limits(), store_scope, MAX_CHECKPOINT_BYTES, &mut reopened)
            .unwrap();
    assert_eq!(recovered.acknowledgement(), 1);
    assert_eq!(recovered.store_revision(), 1);
    assert!(!recovered.can_export_authoritatively());
    let before = fs::read(directory.path().join("replica.head")).unwrap();
    assert_eq!(
        recovered.apply_event(home(1), event, &mut reopened),
        DurableReplicaPublicationV1::Published(ReplicaApplyV1::Replay)
    );
    assert_eq!(
        fs::read(directory.path().join("replica.head")).unwrap(),
        before
    );
}
