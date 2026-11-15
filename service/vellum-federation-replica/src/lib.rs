//! Disclosure-bound direct-home replica journal for Vellum workspaces.

#![forbid(unsafe_code)]

pub mod checkpoint;

pub use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1,
};
use vellum_federation_protocol::{DisclosureProfileV1, FederatedWorkspaceRefV1, VellumHomeIdV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplicaLimitsV1 {
    pub max_events: usize,
    pub max_event_bytes: usize,
    pub max_snapshot_bytes: usize,
    pub max_retained_bytes: usize,
}

impl ReplicaLimitsV1 {
    fn valid(self) -> bool {
        self.max_events != 0
            && self.max_event_bytes != 0
            && self.max_snapshot_bytes != 0
            && self.max_retained_bytes != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicaApplyV1 {
    Applied,
    Replay,
    SnapshotApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicaRefusalV1 {
    InvalidLimits,
    SelfReplica,
    NonHomeSource,
    WrongWorkspace,
    DisclosureMismatch,
    InvalidEvent,
    InvalidSnapshot,
    Gap { expected: u64, observed: u64 },
    Divergence,
    HistoryUnavailable,
    SnapshotRollback,
    Capacity,
    SequenceExhausted,
    MalformedCheckpoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplicaCheckpointV1 {
    pub subscriber: VellumHomeIdV1,
    pub workspace: FederatedWorkspaceRefV1,
    pub disclosure: DisclosureProfileV1,
    pub next_sequence: u64,
    pub snapshot: Option<AuthoritativeWorkspaceSnapshotV1>,
    pub events: Vec<AuthoritativeWorkspaceEventV1>,
}

pub struct ReplicaJournalV1 {
    subscriber: VellumHomeIdV1,
    workspace: FederatedWorkspaceRefV1,
    disclosure: DisclosureProfileV1,
    next_sequence: u64,
    retained_bytes: usize,
    limits: ReplicaLimitsV1,
    snapshot: Option<AuthoritativeWorkspaceSnapshotV1>,
    events: Vec<AuthoritativeWorkspaceEventV1>,
}

impl ReplicaJournalV1 {
    pub fn new(
        subscriber: VellumHomeIdV1,
        workspace: FederatedWorkspaceRefV1,
        disclosure: DisclosureProfileV1,
        limits: ReplicaLimitsV1,
    ) -> Result<Self, ReplicaRefusalV1> {
        if !limits.valid() {
            return Err(ReplicaRefusalV1::InvalidLimits);
        }
        if subscriber == workspace.home {
            return Err(ReplicaRefusalV1::SelfReplica);
        }
        Ok(Self {
            subscriber,
            workspace,
            disclosure,
            next_sequence: 1,
            retained_bytes: 0,
            limits,
            snapshot: None,
            events: Vec::new(),
        })
    }

    pub fn recover(
        checkpoint: ReplicaCheckpointV1,
        limits: ReplicaLimitsV1,
    ) -> Result<Self, ReplicaRefusalV1> {
        let mut journal = Self::new(
            checkpoint.subscriber,
            checkpoint.workspace,
            checkpoint.disclosure,
            limits,
        )?;
        let mut retained_bytes = 0_usize;
        let mut expected = 1_u64;
        if let Some(snapshot) = &checkpoint.snapshot {
            if snapshot.workspace != checkpoint.workspace
                || snapshot.disclosure != checkpoint.disclosure
                || snapshot.bytes().len() > limits.max_snapshot_bytes
            {
                return Err(ReplicaRefusalV1::MalformedCheckpoint);
            }
            retained_bytes = snapshot.bytes().len();
            expected = snapshot
                .base_sequence
                .checked_add(1)
                .ok_or(ReplicaRefusalV1::MalformedCheckpoint)?;
        }
        if checkpoint.events.len() > limits.max_events {
            return Err(ReplicaRefusalV1::MalformedCheckpoint);
        }
        for event in &checkpoint.events {
            if event.workspace != checkpoint.workspace
                || event.disclosure != checkpoint.disclosure
                || event.sequence != expected
                || event.bytes().len() > limits.max_event_bytes
            {
                return Err(ReplicaRefusalV1::MalformedCheckpoint);
            }
            retained_bytes = retained_bytes
                .checked_add(event.bytes().len())
                .filter(|bytes| *bytes <= limits.max_retained_bytes)
                .ok_or(ReplicaRefusalV1::MalformedCheckpoint)?;
            expected = expected
                .checked_add(1)
                .ok_or(ReplicaRefusalV1::MalformedCheckpoint)?;
        }
        if retained_bytes > limits.max_retained_bytes || checkpoint.next_sequence != expected {
            return Err(ReplicaRefusalV1::MalformedCheckpoint);
        }
        journal.next_sequence = expected;
        journal.retained_bytes = retained_bytes;
        journal.snapshot = checkpoint.snapshot;
        journal.events = checkpoint.events;
        Ok(journal)
    }

    pub fn apply(
        &mut self,
        authenticated_sender: VellumHomeIdV1,
        event: AuthoritativeWorkspaceEventV1,
    ) -> Result<ReplicaApplyV1, ReplicaRefusalV1> {
        self.verify_scope(authenticated_sender, event.workspace, event.disclosure)?;
        if event.bytes().len() > self.limits.max_event_bytes {
            return Err(ReplicaRefusalV1::InvalidEvent);
        }
        if event.sequence < self.next_sequence {
            if self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| event.sequence <= snapshot.base_sequence)
            {
                return Err(ReplicaRefusalV1::HistoryUnavailable);
            }
            return self
                .events
                .iter()
                .find(|retained| retained.sequence == event.sequence)
                .map_or(Err(ReplicaRefusalV1::Divergence), |retained| {
                    if retained == &event {
                        Ok(ReplicaApplyV1::Replay)
                    } else {
                        Err(ReplicaRefusalV1::Divergence)
                    }
                });
        }
        if event.sequence > self.next_sequence {
            return Err(ReplicaRefusalV1::Gap {
                expected: self.next_sequence,
                observed: event.sequence,
            });
        }
        if self.events.len() == self.limits.max_events
            || self
                .retained_bytes
                .checked_add(event.bytes().len())
                .is_none_or(|bytes| bytes > self.limits.max_retained_bytes)
        {
            return Err(ReplicaRefusalV1::Capacity);
        }
        let next = self
            .next_sequence
            .checked_add(1)
            .ok_or(ReplicaRefusalV1::SequenceExhausted)?;
        self.retained_bytes += event.bytes().len();
        self.events.push(event);
        self.next_sequence = next;
        Ok(ReplicaApplyV1::Applied)
    }

    pub fn apply_snapshot(
        &mut self,
        authenticated_sender: VellumHomeIdV1,
        snapshot: AuthoritativeWorkspaceSnapshotV1,
    ) -> Result<ReplicaApplyV1, ReplicaRefusalV1> {
        self.verify_scope(
            authenticated_sender,
            snapshot.workspace,
            snapshot.disclosure,
        )?;
        if snapshot.bytes().len() > self.limits.max_snapshot_bytes {
            return Err(ReplicaRefusalV1::InvalidSnapshot);
        }
        if let Some(retained) = &self.snapshot
            && retained.base_sequence == snapshot.base_sequence
        {
            return if retained == &snapshot {
                Ok(ReplicaApplyV1::Replay)
            } else {
                Err(ReplicaRefusalV1::Divergence)
            };
        }
        if snapshot.base_sequence < self.acknowledgement() {
            return Err(ReplicaRefusalV1::SnapshotRollback);
        }
        if snapshot.bytes().len() > self.limits.max_retained_bytes {
            return Err(ReplicaRefusalV1::Capacity);
        }
        let next = snapshot
            .base_sequence
            .checked_add(1)
            .ok_or(ReplicaRefusalV1::SequenceExhausted)?;
        self.retained_bytes = snapshot.bytes().len();
        self.next_sequence = next;
        self.events.clear();
        self.snapshot = Some(snapshot);
        Ok(ReplicaApplyV1::SnapshotApplied)
    }

    pub fn acknowledgement(&self) -> u64 {
        self.next_sequence - 1
    }

    pub fn checkpoint(&self) -> ReplicaCheckpointV1 {
        ReplicaCheckpointV1 {
            subscriber: self.subscriber,
            workspace: self.workspace,
            disclosure: self.disclosure,
            next_sequence: self.next_sequence,
            snapshot: self.snapshot.clone(),
            events: self.events.clone(),
        }
    }

    pub fn can_export_authoritatively(&self) -> bool {
        false
    }

    fn verify_scope(
        &self,
        authenticated_sender: VellumHomeIdV1,
        workspace: FederatedWorkspaceRefV1,
        disclosure: DisclosureProfileV1,
    ) -> Result<(), ReplicaRefusalV1> {
        if authenticated_sender != self.workspace.home {
            return Err(ReplicaRefusalV1::NonHomeSource);
        }
        if workspace != self.workspace {
            return Err(ReplicaRefusalV1::WrongWorkspace);
        }
        if disclosure != self.disclosure {
            return Err(ReplicaRefusalV1::DisclosureMismatch);
        }
        Ok(())
    }
}
