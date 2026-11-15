//! Classified composition from native federation edits to Vellum's ordinary
//! durable service boundary.
//!
//! One instance binds one authenticated remote actor and federated document to
//! one already authenticated Vellum client and document. Federation input can
//! select neither another client nor another document. The adapter accepts one
//! canonical bounded replacement, uses the federation request identity as both
//! Vellum request and durable-operation correlation, and reports success only
//! after Vellum returns durable persistence evidence.

#![forbid(unsafe_code)]

use core::fmt;
use vellum_federation_edit_port::{AuthoritativeEditPortV1, EditExecutionOutcomeV1};
use vellum_federation_protocol::{
    EditRefusalClassV1, FederatedActorRefV1, FederatedDocumentRefV1, RemoteEditReplyV1,
    RemoteEditRequestV1, SourceRevisionV1,
};
use vellum_protocol::{
    ClientId, DurableEffectIntentV1, DurableEffectReceiptV1, DurableExecuteOutcomeV1,
    DurableExecuteRequestV1, DurableOperationRefV1, EditPersistenceV1, RefusalV1, RequestId,
    ServiceAccessErrorV1, ServicePort, WorkspaceDocumentIdV1,
};

const INTENT_VERSION: u8 = 1;
const INTENT_HEADER_BYTES: usize = 1 + 8 + 8 + 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VellumBindingRefusalV1 {
    InvalidIntentLimit,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VellumEditBindingV1 {
    actor: FederatedActorRefV1,
    document: FederatedDocumentRefV1,
    client: ClientId,
    local_document: WorkspaceDocumentIdV1,
    request_namespace: [u8; 8],
    max_insert_bytes: usize,
}

impl VellumEditBindingV1 {
    pub fn new(
        actor: FederatedActorRefV1,
        document: FederatedDocumentRefV1,
        client: ClientId,
        local_document: WorkspaceDocumentIdV1,
        request_namespace: [u8; 8],
        max_insert_bytes: usize,
    ) -> Result<Self, VellumBindingRefusalV1> {
        if max_insert_bytes == 0 || max_insert_bytes > u32::MAX as usize {
            return Err(VellumBindingRefusalV1::InvalidIntentLimit);
        }
        Ok(Self {
            actor,
            document,
            client,
            local_document,
            request_namespace,
            max_insert_bytes,
        })
    }
}

impl fmt::Debug for VellumEditBindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VellumEditBindingV1")
            .field("actor", &self.actor)
            .field("document", &self.document)
            .field("local_document", &self.local_document)
            .field("max_insert_bytes", &self.max_insert_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementIntentV1 {
    pub start: u64,
    pub end: u64,
    pub inserted: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementIntentErrorV1 {
    Invalid,
    Limit,
}

pub fn encode_replacement_intent_v1(
    intent: &ReplacementIntentV1,
    max_insert_bytes: usize,
) -> Result<Vec<u8>, ReplacementIntentErrorV1> {
    if intent.start > intent.end || intent.inserted.len() > max_insert_bytes {
        return Err(if intent.inserted.len() > max_insert_bytes {
            ReplacementIntentErrorV1::Limit
        } else {
            ReplacementIntentErrorV1::Invalid
        });
    }
    let length =
        u32::try_from(intent.inserted.len()).map_err(|_| ReplacementIntentErrorV1::Limit)?;
    let mut bytes = Vec::with_capacity(INTENT_HEADER_BYTES + intent.inserted.len());
    bytes.push(INTENT_VERSION);
    bytes.extend_from_slice(&intent.start.to_be_bytes());
    bytes.extend_from_slice(&intent.end.to_be_bytes());
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(intent.inserted.as_bytes());
    Ok(bytes)
}

pub fn decode_replacement_intent_v1(
    bytes: &[u8],
    max_insert_bytes: usize,
) -> Result<ReplacementIntentV1, ReplacementIntentErrorV1> {
    if bytes.len() < INTENT_HEADER_BYTES || bytes[0] != INTENT_VERSION {
        return Err(ReplacementIntentErrorV1::Invalid);
    }
    let start = u64::from_be_bytes(bytes[1..9].try_into().expect("fixed start field"));
    let end = u64::from_be_bytes(bytes[9..17].try_into().expect("fixed end field"));
    let length = u32::from_be_bytes(bytes[17..21].try_into().expect("fixed length field")) as usize;
    if length > max_insert_bytes {
        return Err(ReplacementIntentErrorV1::Limit);
    }
    if start > end || bytes.len() != INTENT_HEADER_BYTES + length {
        return Err(ReplacementIntentErrorV1::Invalid);
    }
    let inserted = core::str::from_utf8(&bytes[INTENT_HEADER_BYTES..])
        .map_err(|_| ReplacementIntentErrorV1::Invalid)?
        .to_owned();
    Ok(ReplacementIntentV1 {
        start,
        end,
        inserted,
    })
}

pub struct VellumAuthoritativeEditV1<P> {
    binding: VellumEditBindingV1,
    service: P,
    closed: bool,
}

impl<P> VellumAuthoritativeEditV1<P> {
    pub fn new(binding: VellumEditBindingV1, service: P) -> Self {
        Self {
            binding,
            service,
            closed: false,
        }
    }

    pub fn service(&self) -> &P {
        &self.service
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn into_service(self) -> P {
        self.service
    }
}

impl<P> fmt::Debug for VellumAuthoritativeEditV1<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VellumAuthoritativeEditV1")
            .field("binding", &self.binding)
            .field("closed", &self.closed)
            .field("service", &"<configured>")
            .finish()
    }
}

impl<P: ServicePort> AuthoritativeEditPortV1 for VellumAuthoritativeEditV1<P> {
    fn execute(&mut self, request: &RemoteEditRequestV1) -> EditExecutionOutcomeV1 {
        if self.closed {
            return stable_refusal(EditRefusalClassV1::Unavailable);
        }
        if request.actor != self.binding.actor || request.document != self.binding.document {
            return stable_refusal(EditRefusalClassV1::Unauthorized);
        }
        let intent =
            match decode_replacement_intent_v1(request.intent(), self.binding.max_insert_bytes) {
                Ok(intent) => intent,
                Err(_) => return stable_refusal(EditRefusalClassV1::InvalidIntent),
            };
        let request_bytes = request.request_id.as_bytes();
        if request_bytes[..8] != self.binding.request_namespace {
            return stable_refusal(EditRefusalClassV1::InvalidIntent);
        }
        let native_request_id = u64::from_be_bytes(
            request_bytes[8..]
                .try_into()
                .expect("fixed native request suffix"),
        );
        let Some(native_request_id) = RequestId::new(native_request_id) else {
            return stable_refusal(EditRefusalClassV1::InvalidIntent);
        };
        let operation = DurableOperationRefV1::new(request_bytes)
            .expect("federation edit request identities are nonzero");
        let native_request = DurableExecuteRequestV1 {
            operation,
            client: self.binding.client,
            request: native_request_id,
            intent: DurableEffectIntentV1::ReplaceDocumentAndSave {
                document: self.binding.local_document,
                expected_revision: request.expected_revision.get(),
                start: intent.start,
                end: intent.end,
                inserted: intent.inserted,
            },
        };
        match self.service.execute_durable(native_request) {
            Err(ServiceAccessErrorV1::NotSent) => EditExecutionOutcomeV1::DefinitelyNotExecuted,
            Err(_) => EditExecutionOutcomeV1::OutcomeUnknown,
            Ok(DurableExecuteOutcomeV1::Completed(
                DurableEffectReceiptV1::DocumentReplacedAndSaved(receipt),
            )) if receipt.document == self.binding.local_document
                && receipt.source_revision > request.expected_revision.get()
                && matches!(receipt.persistence, EditPersistenceV1::Durable(_)) =>
            {
                let Some(resulting_revision) = SourceRevisionV1::new(receipt.source_revision)
                else {
                    return EditExecutionOutcomeV1::OutcomeUnknown;
                };
                EditExecutionOutcomeV1::Reply(RemoteEditReplyV1::Applied { resulting_revision })
            }
            Ok(DurableExecuteOutcomeV1::Completed(_)) => EditExecutionOutcomeV1::OutcomeUnknown,
            Ok(DurableExecuteOutcomeV1::Retired) => EditExecutionOutcomeV1::OutcomeUnknown,
            Ok(DurableExecuteOutcomeV1::Refused(refusal)) => map_refusal(refusal),
        }
    }
}

fn stable_refusal(class: EditRefusalClassV1) -> EditExecutionOutcomeV1 {
    EditExecutionOutcomeV1::Reply(RemoteEditReplyV1::Refused(class))
}

fn map_refusal(refusal: RefusalV1) -> EditExecutionOutcomeV1 {
    use RefusalV1::{
        AuthorityDenied, AuthorityUnavailable, CounterExhausted, EditorCapacity, EditorClosed,
        EditorConflict, EditorIncomplete, EditorUnavailable, InputLimit, InvalidRange,
        NoActiveDocument, RequestConflict, Stale, UnknownClient, UnknownDocument,
    };
    match refusal {
        AuthorityDenied | UnknownClient | UnknownDocument | NoActiveDocument => {
            stable_refusal(EditRefusalClassV1::Unauthorized)
        }
        Stale => stable_refusal(EditRefusalClassV1::StaleRevision),
        InvalidRange | InputLimit => stable_refusal(EditRefusalClassV1::InvalidIntent),
        RequestConflict => EditExecutionOutcomeV1::OutcomeUnknown,
        AuthorityUnavailable | EditorClosed | EditorCapacity | EditorConflict
        | EditorIncomplete | EditorUnavailable | CounterExhausted => {
            stable_refusal(EditRefusalClassV1::Unavailable)
        }
        _ => stable_refusal(EditRefusalClassV1::Unavailable),
    }
}
