//! Private local Unix directory storage for one Vellum replica checkpoint.
//!
//! The adapter serializes compare-exchange offers through a mode-0600 pending
//! file, atomically replaces one canonical head record, synchronizes the file
//! and directory, and validates the exact published record before returning a
//! committed receipt. It grants no Vellum or federation authority.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use vellum_federation_replica_store_port::{
    ReplicaStoreLoadV1, ReplicaStoreOfferV1, ReplicaStorePortV1, ReplicaStoreRecordV1,
    ReplicaStoreScopeV1,
};

const MAGIC: &[u8; 8] = b"VLMFRD1\0";
const HEAD: &str = "replica.head";
const LOCK: &str = "replica.lock";
const PENDING: &str = "replica.pending";
const RECORD_OVERHEAD: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryReplicaOpenRefusalV1 {
    InvalidConfiguration,
    UnsafeDirectory,
    PendingPublication,
    Unavailable,
}

pub struct DirectoryReplicaStoreV1 {
    directory: PathBuf,
    scope: ReplicaStoreScopeV1,
    max_checkpoint_bytes: usize,
}

impl DirectoryReplicaStoreV1 {
    pub fn open(
        directory: impl AsRef<Path>,
        scope: ReplicaStoreScopeV1,
        max_checkpoint_bytes: usize,
    ) -> Result<Self, DirectoryReplicaOpenRefusalV1> {
        let store = Self::prepare(directory.as_ref(), scope, max_checkpoint_bytes)?;
        if resource_may_exist(&store.lock_path()) || resource_may_exist(&store.pending_path()) {
            return Err(DirectoryReplicaOpenRefusalV1::PendingPublication);
        }
        Ok(store)
    }

    /// Recovers only recognized interruption residue after the caller has
    /// externally established that no live owner remains. The canonical head
    /// is validated first and remains the sole commit point; pending bytes are
    /// never promoted. Unsafe resource kinds or modes refuse without cleanup.
    pub fn recover_interrupted_after_exclusive_shutdown(
        directory: impl AsRef<Path>,
        scope: ReplicaStoreScopeV1,
        max_checkpoint_bytes: usize,
    ) -> Result<Self, DirectoryReplicaOpenRefusalV1> {
        let store = Self::prepare(directory.as_ref(), scope, max_checkpoint_bytes)?;
        let pending = validate_residue_resource(&store.pending_path())?;
        let lock = validate_residue_resource(&store.lock_path())?;
        if pending {
            fs::remove_file(store.pending_path())
                .map_err(|_| DirectoryReplicaOpenRefusalV1::Unavailable)?;
        }
        if lock {
            fs::remove_file(store.lock_path())
                .map_err(|_| DirectoryReplicaOpenRefusalV1::Unavailable)?;
        }
        if pending || lock {
            sync_directory(&store.directory)
                .map_err(|_| DirectoryReplicaOpenRefusalV1::Unavailable)?;
        }
        Ok(store)
    }

    fn prepare(
        directory: &Path,
        scope: ReplicaStoreScopeV1,
        max_checkpoint_bytes: usize,
    ) -> Result<Self, DirectoryReplicaOpenRefusalV1> {
        if max_checkpoint_bytes == 0
            || max_checkpoint_bytes.checked_add(RECORD_OVERHEAD).is_none()
            || !directory.is_absolute()
        {
            return Err(DirectoryReplicaOpenRefusalV1::InvalidConfiguration);
        }
        match fs::symlink_metadata(directory) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o777 != 0o700
                {
                    return Err(DirectoryReplicaOpenRefusalV1::UnsafeDirectory);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(directory)
                    .map_err(|_| DirectoryReplicaOpenRefusalV1::Unavailable)?;
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                    .map_err(|_| DirectoryReplicaOpenRefusalV1::Unavailable)?;
                sync_directory(directory)
                    .map_err(|_| DirectoryReplicaOpenRefusalV1::Unavailable)?;
            }
            Err(_) => return Err(DirectoryReplicaOpenRefusalV1::Unavailable),
        }
        let store = Self {
            directory: directory.to_path_buf(),
            scope,
            max_checkpoint_bytes,
        };
        match store.read_head() {
            Ok(_) => Ok(store),
            Err(ReadFailure::Unsafe) => Err(DirectoryReplicaOpenRefusalV1::UnsafeDirectory),
            Err(ReadFailure::Unavailable) => Err(DirectoryReplicaOpenRefusalV1::Unavailable),
        }
    }

    fn head_path(&self) -> PathBuf {
        self.directory.join(HEAD)
    }

    fn pending_path(&self) -> PathBuf {
        self.directory.join(PENDING)
    }

    fn lock_path(&self) -> PathBuf {
        self.directory.join(LOCK)
    }

    fn read_head(&self) -> Result<Option<ReplicaStoreRecordV1>, ReadFailure> {
        let path = self.head_path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ReadFailure::Unavailable),
        };
        if !metadata.file_type().is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(ReadFailure::Unsafe);
        }
        let maximum = self
            .max_checkpoint_bytes
            .checked_add(RECORD_OVERHEAD)
            .ok_or(ReadFailure::Unavailable)?;
        let length = usize::try_from(metadata.len()).map_err(|_| ReadFailure::Unavailable)?;
        if length > maximum {
            return Err(ReadFailure::Unavailable);
        }
        let mut bytes = Vec::with_capacity(length);
        File::open(path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|_| ReadFailure::Unavailable)?;
        decode_record(&bytes, self.scope, self.max_checkpoint_bytes)
            .map(Some)
            .ok_or(ReadFailure::Unavailable)
    }

    fn remove_precommit_residue(&self) {
        let _ = fs::remove_file(self.pending_path());
        let _ = fs::remove_file(self.lock_path());
        let _ = sync_directory(&self.directory);
    }

    fn release_lock(&self) -> io::Result<()> {
        fs::remove_file(self.lock_path())?;
        sync_directory(&self.directory)
    }
}

impl ReplicaStorePortV1 for DirectoryReplicaStoreV1 {
    fn load(&mut self, scope: ReplicaStoreScopeV1) -> ReplicaStoreLoadV1 {
        if scope != self.scope
            || resource_may_exist(&self.lock_path())
            || resource_may_exist(&self.pending_path())
        {
            return ReplicaStoreLoadV1::Unavailable;
        }
        match self.read_head() {
            Ok(None) => ReplicaStoreLoadV1::Missing,
            Ok(Some(record)) => ReplicaStoreLoadV1::Present(record),
            Err(_) => ReplicaStoreLoadV1::Unavailable,
        }
    }

    fn compare_exchange(
        &mut self,
        expected_revision: Option<u64>,
        proposed: &ReplicaStoreRecordV1,
    ) -> ReplicaStoreOfferV1 {
        let revision_is_next = match expected_revision {
            None => proposed.revision == 0,
            Some(expected) => expected.checked_add(1) == Some(proposed.revision),
        };
        if proposed.scope != self.scope
            || proposed.checkpoint().is_empty()
            || proposed.checkpoint().len() > self.max_checkpoint_bytes
            || !revision_is_next
        {
            return ReplicaStoreOfferV1::NotCommitted;
        }
        let lock_path = self.lock_path();
        let lock = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(_) => return ReplicaStoreOfferV1::OutcomeUnknown,
        };
        if fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).is_err()
            || lock.sync_all().is_err()
            || sync_directory(&self.directory).is_err()
        {
            self.remove_precommit_residue();
            return ReplicaStoreOfferV1::NotCommitted;
        }
        let current_revision = match self.read_head() {
            Ok(record) => record.map(|record| record.revision),
            Err(_) => {
                self.remove_precommit_residue();
                return ReplicaStoreOfferV1::NotCommitted;
            }
        };
        if current_revision != expected_revision {
            self.remove_precommit_residue();
            return ReplicaStoreOfferV1::NotCommitted;
        }
        let pending_path = self.pending_path();
        let mut pending = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending_path)
        {
            Ok(file) => file,
            Err(_) => {
                self.remove_precommit_residue();
                return ReplicaStoreOfferV1::NotCommitted;
            }
        };
        if fs::set_permissions(&pending_path, fs::Permissions::from_mode(0o600)).is_err() {
            self.remove_precommit_residue();
            return ReplicaStoreOfferV1::NotCommitted;
        }
        let bytes = encode_record(proposed);
        if pending
            .write_all(&bytes)
            .and_then(|()| pending.sync_all())
            .is_err()
        {
            self.remove_precommit_residue();
            return ReplicaStoreOfferV1::NotCommitted;
        }
        drop(pending);
        if fs::rename(&pending_path, self.head_path()).is_err() {
            self.remove_precommit_residue();
            return ReplicaStoreOfferV1::NotCommitted;
        }
        if sync_directory(&self.directory).is_err() {
            return ReplicaStoreOfferV1::OutcomeUnknown;
        }
        let receipt = match self.read_head() {
            Ok(Some(receipt)) if receipt == *proposed => receipt,
            _ => return ReplicaStoreOfferV1::OutcomeUnknown,
        };
        if self.release_lock().is_err() {
            ReplicaStoreOfferV1::OutcomeUnknown
        } else {
            ReplicaStoreOfferV1::Committed(receipt)
        }
    }
}

impl fmt::Debug for DirectoryReplicaStoreV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectoryReplicaStoreV1")
            .field("directory", &"<redacted>")
            .field("scope", &self.scope)
            .field("max_checkpoint_bytes", &self.max_checkpoint_bytes)
            .finish()
    }
}

#[derive(Clone, Copy)]
enum ReadFailure {
    Unsafe,
    Unavailable,
}

fn encode_record(record: &ReplicaStoreRecordV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RECORD_OVERHEAD + record.checkpoint().len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&record.scope.as_bytes());
    bytes.extend_from_slice(&record.revision.to_be_bytes());
    bytes.extend_from_slice(&(record.checkpoint().len() as u64).to_be_bytes());
    bytes.extend_from_slice(record.checkpoint());
    bytes.extend_from_slice(&checksum(&bytes).to_be_bytes());
    bytes
}

fn decode_record(
    bytes: &[u8],
    scope: ReplicaStoreScopeV1,
    max_checkpoint_bytes: usize,
) -> Option<ReplicaStoreRecordV1> {
    if bytes.len() < RECORD_OVERHEAD || &bytes[..8] != MAGIC {
        return None;
    }
    let payload_end = bytes.len().checked_sub(8)?;
    let expected_checksum = u64::from_be_bytes(bytes[payload_end..].try_into().ok()?);
    if checksum(&bytes[..payload_end]) != expected_checksum {
        return None;
    }
    let stored_scope = ReplicaStoreScopeV1::new(bytes[8..40].try_into().ok()?)?;
    if stored_scope != scope {
        return None;
    }
    let revision = u64::from_be_bytes(bytes[40..48].try_into().ok()?);
    let checkpoint_length =
        usize::try_from(u64::from_be_bytes(bytes[48..56].try_into().ok()?)).ok()?;
    if checkpoint_length == 0
        || checkpoint_length > max_checkpoint_bytes
        || 56_usize.checked_add(checkpoint_length)? != payload_end
    {
        return None;
    }
    ReplicaStoreRecordV1::new(
        stored_scope,
        revision,
        bytes[56..payload_end].to_vec(),
        max_checkpoint_bytes,
    )
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |state, byte| {
        (state ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn sync_directory(directory: &Path) -> io::Result<()> {
    File::open(directory)?.sync_all()
}

fn resource_may_exist(path: &Path) -> bool {
    !matches!(
        fs::symlink_metadata(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    )
}

fn validate_residue_resource(path: &Path) -> Result<bool, DirectoryReplicaOpenRefusalV1> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(DirectoryReplicaOpenRefusalV1::Unavailable),
        Ok(metadata)
            if metadata.file_type().is_file() && metadata.permissions().mode() & 0o777 == 0o600 =>
        {
            Ok(true)
        }
        Ok(_) => Err(DirectoryReplicaOpenRefusalV1::UnsafeDirectory),
    }
}
