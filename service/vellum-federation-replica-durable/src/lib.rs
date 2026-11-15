//! Commit-gated durable owner for one Vellum federation replica journal.
//!
//! The owner publishes canonical replica checkpoints. It does not edit, save,
//! persist source, allocate revisions, or export authoritative document state.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, VellumHomeIdV1,
};
use vellum_federation_replica::{
    ReplicaApplyV1, ReplicaJournalV1, ReplicaLimitsV1, ReplicaRefusalV1,
    checkpoint::{decode_checkpoint_v1, encode_checkpoint_v1},
};
use vellum_federation_replica_store_port::{
    ReplicaStoreLoadV1, ReplicaStoreOfferV1, ReplicaStorePortV1, ReplicaStoreRecordV1,
    ReplicaStoreScopeV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableReplicaOpenRefusalV1 {
    InvalidCheckpointLimit,
    Missing,
    StoreUnavailable,
    StoreRefused,
    OutcomeUnknown,
    MalformedCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableReplicaPublicationV1 {
    Published(ReplicaApplyV1),
    Refused(ReplicaRefusalV1),
    StoreRefused,
    OutcomeUnknown,
    Fenced,
    RevisionExhausted,
    CheckpointLimit,
}

pub struct DurableReplicaV1 {
    journal: ReplicaJournalV1,
    limits: ReplicaLimitsV1,
    scope: ReplicaStoreScopeV1,
    store_revision: u64,
    max_checkpoint_bytes: usize,
    fenced: bool,
}

impl DurableReplicaV1 {
    pub fn initialize<S: ReplicaStorePortV1>(
        journal: ReplicaJournalV1,
        limits: ReplicaLimitsV1,
        scope: ReplicaStoreScopeV1,
        max_checkpoint_bytes: usize,
        store: &mut S,
    ) -> Result<Self, DurableReplicaOpenRefusalV1> {
        if max_checkpoint_bytes == 0 {
            return Err(DurableReplicaOpenRefusalV1::InvalidCheckpointLimit);
        }
        let bytes = encode_checkpoint_v1(&journal.checkpoint(), limits)
            .map_err(|_| DurableReplicaOpenRefusalV1::MalformedCheckpoint)?;
        let proposed = ReplicaStoreRecordV1::new(scope, 0, bytes, max_checkpoint_bytes)
            .ok_or(DurableReplicaOpenRefusalV1::InvalidCheckpointLimit)?;
        match store.compare_exchange(None, &proposed) {
            ReplicaStoreOfferV1::Committed(receipt) if receipt == proposed => Ok(Self {
                journal,
                limits,
                scope,
                store_revision: 0,
                max_checkpoint_bytes,
                fenced: false,
            }),
            ReplicaStoreOfferV1::NotCommitted => Err(DurableReplicaOpenRefusalV1::StoreRefused),
            ReplicaStoreOfferV1::OutcomeUnknown => Err(DurableReplicaOpenRefusalV1::OutcomeUnknown),
            ReplicaStoreOfferV1::Committed(_) => Err(DurableReplicaOpenRefusalV1::OutcomeUnknown),
        }
    }

    pub fn recover<S: ReplicaStorePortV1>(
        limits: ReplicaLimitsV1,
        scope: ReplicaStoreScopeV1,
        max_checkpoint_bytes: usize,
        store: &mut S,
    ) -> Result<Self, DurableReplicaOpenRefusalV1> {
        if max_checkpoint_bytes == 0 {
            return Err(DurableReplicaOpenRefusalV1::InvalidCheckpointLimit);
        }
        let record = match store.load(scope) {
            ReplicaStoreLoadV1::Missing => return Err(DurableReplicaOpenRefusalV1::Missing),
            ReplicaStoreLoadV1::Unavailable => {
                return Err(DurableReplicaOpenRefusalV1::StoreUnavailable);
            }
            ReplicaStoreLoadV1::Present(record) => record,
        };
        if record.scope != scope || record.checkpoint().len() > max_checkpoint_bytes {
            return Err(DurableReplicaOpenRefusalV1::MalformedCheckpoint);
        }
        let checkpoint = decode_checkpoint_v1(record.checkpoint(), limits)
            .map_err(|_| DurableReplicaOpenRefusalV1::MalformedCheckpoint)?;
        let journal = ReplicaJournalV1::recover(checkpoint, limits)
            .map_err(|_| DurableReplicaOpenRefusalV1::MalformedCheckpoint)?;
        Ok(Self {
            journal,
            limits,
            scope,
            store_revision: record.revision,
            max_checkpoint_bytes,
            fenced: false,
        })
    }

    pub fn apply_event<S: ReplicaStorePortV1>(
        &mut self,
        authenticated_sender: VellumHomeIdV1,
        event: AuthoritativeWorkspaceEventV1,
        store: &mut S,
    ) -> DurableReplicaPublicationV1 {
        if self.fenced {
            return DurableReplicaPublicationV1::Fenced;
        }
        let mut candidate = ReplicaJournalV1::recover(self.journal.checkpoint(), self.limits)
            .expect("current journal checkpoint remains valid");
        match candidate.apply(authenticated_sender, event) {
            Ok(ReplicaApplyV1::Replay) => {
                DurableReplicaPublicationV1::Published(ReplicaApplyV1::Replay)
            }
            Ok(applied) => self.publish(candidate, applied, store),
            Err(refusal) => DurableReplicaPublicationV1::Refused(refusal),
        }
    }

    pub fn apply_snapshot<S: ReplicaStorePortV1>(
        &mut self,
        authenticated_sender: VellumHomeIdV1,
        snapshot: AuthoritativeWorkspaceSnapshotV1,
        store: &mut S,
    ) -> DurableReplicaPublicationV1 {
        if self.fenced {
            return DurableReplicaPublicationV1::Fenced;
        }
        let mut candidate = ReplicaJournalV1::recover(self.journal.checkpoint(), self.limits)
            .expect("current journal checkpoint remains valid");
        match candidate.apply_snapshot(authenticated_sender, snapshot) {
            Ok(ReplicaApplyV1::Replay) => {
                DurableReplicaPublicationV1::Published(ReplicaApplyV1::Replay)
            }
            Ok(applied) => self.publish(candidate, applied, store),
            Err(refusal) => DurableReplicaPublicationV1::Refused(refusal),
        }
    }

    pub fn acknowledgement(&self) -> u64 {
        self.journal.acknowledgement()
    }

    pub fn store_revision(&self) -> u64 {
        self.store_revision
    }

    pub fn is_fenced(&self) -> bool {
        self.fenced
    }

    pub fn can_export_authoritatively(&self) -> bool {
        false
    }

    fn publish<S: ReplicaStorePortV1>(
        &mut self,
        candidate: ReplicaJournalV1,
        applied: ReplicaApplyV1,
        store: &mut S,
    ) -> DurableReplicaPublicationV1 {
        let Some(next_revision) = self.store_revision.checked_add(1) else {
            return DurableReplicaPublicationV1::RevisionExhausted;
        };
        let Ok(bytes) = encode_checkpoint_v1(&candidate.checkpoint(), self.limits) else {
            return DurableReplicaPublicationV1::CheckpointLimit;
        };
        let Some(proposed) =
            ReplicaStoreRecordV1::new(self.scope, next_revision, bytes, self.max_checkpoint_bytes)
        else {
            return DurableReplicaPublicationV1::CheckpointLimit;
        };
        match store.compare_exchange(Some(self.store_revision), &proposed) {
            ReplicaStoreOfferV1::Committed(receipt) if receipt == proposed => {
                self.journal = candidate;
                self.store_revision = next_revision;
                DurableReplicaPublicationV1::Published(applied)
            }
            ReplicaStoreOfferV1::NotCommitted => DurableReplicaPublicationV1::StoreRefused,
            ReplicaStoreOfferV1::OutcomeUnknown | ReplicaStoreOfferV1::Committed(_) => {
                self.fenced = true;
                DurableReplicaPublicationV1::OutcomeUnknown
            }
        }
    }
}
