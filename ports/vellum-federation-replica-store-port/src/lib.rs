//! Effect-free storage contract for one Vellum federation replica checkpoint.
//!
//! Store receipts describe checkpoint publication only. They grant no document,
//! edit, save, source, persistence, replication-source, or transport authority.

#![forbid(unsafe_code)]

use core::fmt;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplicaStoreScopeV1([u8; 32]);

impl ReplicaStoreScopeV1 {
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        (bytes != [0; 32]).then_some(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for ReplicaStoreScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReplicaStoreScopeV1(<redacted>)")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ReplicaStoreRecordV1 {
    pub scope: ReplicaStoreScopeV1,
    pub revision: u64,
    checkpoint: Vec<u8>,
}

impl ReplicaStoreRecordV1 {
    pub fn new(
        scope: ReplicaStoreScopeV1,
        revision: u64,
        checkpoint: Vec<u8>,
        max_checkpoint_bytes: usize,
    ) -> Option<Self> {
        (!checkpoint.is_empty()
            && checkpoint.len() <= max_checkpoint_bytes
            && max_checkpoint_bytes != 0)
            .then_some(Self {
                scope,
                revision,
                checkpoint,
            })
    }

    pub fn checkpoint(&self) -> &[u8] {
        &self.checkpoint
    }
}

impl fmt::Debug for ReplicaStoreRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplicaStoreRecordV1")
            .field("scope", &self.scope)
            .field("revision", &self.revision)
            .field("checkpoint_bytes", &self.checkpoint.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplicaStoreLoadV1 {
    Missing,
    Present(ReplicaStoreRecordV1),
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplicaStoreOfferV1 {
    Committed(ReplicaStoreRecordV1),
    NotCommitted,
    OutcomeUnknown,
}

pub trait ReplicaStorePortV1 {
    fn load(&mut self, scope: ReplicaStoreScopeV1) -> ReplicaStoreLoadV1;

    fn compare_exchange(
        &mut self,
        expected_revision: Option<u64>,
        proposed: &ReplicaStoreRecordV1,
    ) -> ReplicaStoreOfferV1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_bounded_and_diagnostics_hide_scope_and_checkpoint() {
        let scope = ReplicaStoreScopeV1::new([7; 32]).unwrap();
        assert!(ReplicaStoreScopeV1::new([0; 32]).is_none());
        assert!(ReplicaStoreRecordV1::new(scope, 0, Vec::new(), 8).is_none());
        assert!(ReplicaStoreRecordV1::new(scope, 0, vec![1; 9], 8).is_none());
        let record = ReplicaStoreRecordV1::new(scope, 3, vec![9; 8], 8).unwrap();
        let diagnostic = format!("{record:?}");
        assert!(diagnostic.contains("checkpoint_bytes: 8"));
        assert!(!diagnostic.contains("9, 9"));
        assert!(format!("{scope:?}").contains("<redacted>"));
    }
}
