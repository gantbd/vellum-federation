//! Effect-free boundary for authoritative Vellum edit execution.
//!
//! Implementations must revalidate current Vellum authority before first
//! execution. This port owns no credentials, transport, persistence, retry,
//! workspace policy, edit semantics, or participant-kind policy.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{RemoteEditReplyV1, RemoteEditRequestV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditExecutionOutcomeV1 {
    Reply(RemoteEditReplyV1),
    DefinitelyNotExecuted,
    OutcomeUnknown,
}

pub trait AuthoritativeEditPortV1 {
    fn execute(&mut self, request: &RemoteEditRequestV1) -> EditExecutionOutcomeV1;
}
