//! Effect boundary for closing one established Vellum federation transport.

#![forbid(unsafe_code)]

/// A transport owner whose close operation is irreversible.
///
/// Implementations must make repeated close calls safe and must not turn close
/// completion into evidence of remote receipt, replication, edit, or save.
pub trait FederationTransportLifecyclePortV1 {
    type Error;

    fn close(&mut self) -> Result<(), Self::Error>;
}
