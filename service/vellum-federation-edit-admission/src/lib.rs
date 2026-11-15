//! Exact home receipts and client uncertainty for remote Vellum edits.

#![forbid(unsafe_code)]

use vellum_federation_protocol::{
    EditRequestIdV1, RemoteEditReplyV1, RemoteEditRequestV1, VellumHomeIdV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditAdmissionLimitsV1 {
    pub max_receipts: usize,
    pub max_intent_bytes: usize,
}

impl EditAdmissionLimitsV1 {
    fn valid(self) -> bool {
        self.max_receipts != 0 && self.max_intent_bytes != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditAdmissionApplyV1 {
    Applied(RemoteEditReplyV1),
    Replay(RemoteEditReplyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditAdmissionRefusalV1 {
    InvalidLimits,
    NotWorkspaceHome,
    SelfAdmission,
    ActorSourceMismatch,
    IntentLimit,
    ReceiptCapacity,
    RequestConflict,
    InvalidReply,
    MalformedCheckpoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditReceiptV1 {
    pub sender: VellumHomeIdV1,
    pub request: RemoteEditRequestV1,
    pub reply: RemoteEditReplyV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditAdmissionCheckpointV1 {
    pub local_home: VellumHomeIdV1,
    pub receipts: Vec<EditReceiptV1>,
}

pub struct EditAdmissionHomeV1 {
    local_home: VellumHomeIdV1,
    limits: EditAdmissionLimitsV1,
    receipts: Vec<EditReceiptV1>,
}

impl EditAdmissionHomeV1 {
    pub fn new(
        local_home: VellumHomeIdV1,
        limits: EditAdmissionLimitsV1,
    ) -> Result<Self, EditAdmissionRefusalV1> {
        if !limits.valid() {
            return Err(EditAdmissionRefusalV1::InvalidLimits);
        }
        Ok(Self {
            local_home,
            limits,
            receipts: Vec::new(),
        })
    }

    pub fn recover(
        checkpoint: EditAdmissionCheckpointV1,
        limits: EditAdmissionLimitsV1,
    ) -> Result<Self, EditAdmissionRefusalV1> {
        let mut home = Self::new(checkpoint.local_home, limits)?;
        if checkpoint.receipts.len() > limits.max_receipts {
            return Err(EditAdmissionRefusalV1::MalformedCheckpoint);
        }
        for receipt in checkpoint.receipts {
            home.validate(receipt.sender, &receipt.request)
                .map_err(|_| EditAdmissionRefusalV1::MalformedCheckpoint)?;
            if !valid_reply(&receipt.request, receipt.reply)
                || home.receipts.iter().any(|existing| {
                    existing.sender == receipt.sender
                        && existing.request.request_id == receipt.request.request_id
                })
            {
                return Err(EditAdmissionRefusalV1::MalformedCheckpoint);
            }
            home.receipts.push(receipt);
        }
        Ok(home)
    }

    pub fn preflight(
        &self,
        authenticated_sender: VellumHomeIdV1,
        request: &RemoteEditRequestV1,
    ) -> Result<Option<RemoteEditReplyV1>, EditAdmissionRefusalV1> {
        self.validate(authenticated_sender, request)?;
        if let Some(receipt) = self.receipts.iter().find(|receipt| {
            receipt.sender == authenticated_sender
                && receipt.request.request_id == request.request_id
        }) {
            return if &receipt.request == request {
                Ok(Some(receipt.reply))
            } else {
                Err(EditAdmissionRefusalV1::RequestConflict)
            };
        }
        if self.receipts.len() == self.limits.max_receipts {
            return Err(EditAdmissionRefusalV1::ReceiptCapacity);
        }
        Ok(None)
    }

    pub fn admit<F>(
        &mut self,
        authenticated_sender: VellumHomeIdV1,
        request: RemoteEditRequestV1,
        execute: F,
    ) -> Result<EditAdmissionApplyV1, EditAdmissionRefusalV1>
    where
        F: FnOnce(&RemoteEditRequestV1) -> RemoteEditReplyV1,
    {
        if let Some(reply) = self.preflight(authenticated_sender, &request)? {
            return Ok(EditAdmissionApplyV1::Replay(reply));
        }
        let reply = execute(&request);
        self.record(authenticated_sender, request, reply)
    }

    pub fn record(
        &mut self,
        authenticated_sender: VellumHomeIdV1,
        request: RemoteEditRequestV1,
        reply: RemoteEditReplyV1,
    ) -> Result<EditAdmissionApplyV1, EditAdmissionRefusalV1> {
        if let Some(retained) = self.preflight(authenticated_sender, &request)? {
            return if retained == reply {
                Ok(EditAdmissionApplyV1::Replay(retained))
            } else {
                Err(EditAdmissionRefusalV1::InvalidReply)
            };
        }
        if !valid_reply(&request, reply) {
            return Err(EditAdmissionRefusalV1::InvalidReply);
        }
        self.receipts.push(EditReceiptV1 {
            sender: authenticated_sender,
            request,
            reply,
        });
        Ok(EditAdmissionApplyV1::Applied(reply))
    }

    pub fn checkpoint(&self) -> EditAdmissionCheckpointV1 {
        EditAdmissionCheckpointV1 {
            local_home: self.local_home,
            receipts: self.receipts.clone(),
        }
    }

    pub fn receipt_count(&self) -> usize {
        self.receipts.len()
    }

    fn validate(
        &self,
        authenticated_sender: VellumHomeIdV1,
        request: &RemoteEditRequestV1,
    ) -> Result<(), EditAdmissionRefusalV1> {
        if request.document.workspace.home != self.local_home {
            return Err(EditAdmissionRefusalV1::NotWorkspaceHome);
        }
        if authenticated_sender == self.local_home {
            return Err(EditAdmissionRefusalV1::SelfAdmission);
        }
        if request.actor.home != authenticated_sender {
            return Err(EditAdmissionRefusalV1::ActorSourceMismatch);
        }
        if request.intent().len() > self.limits.max_intent_bytes {
            return Err(EditAdmissionRefusalV1::IntentLimit);
        }
        Ok(())
    }
}

fn valid_reply(request: &RemoteEditRequestV1, reply: RemoteEditReplyV1) -> bool {
    match reply {
        RemoteEditReplyV1::Applied { resulting_revision } => {
            resulting_revision > request.expected_revision
        }
        RemoteEditReplyV1::Refused(_) => true,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditPortOutcomeV1 {
    Reply {
        request_id: EditRequestIdV1,
        reply: RemoteEditReplyV1,
    },
    DefinitelyNotSent,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditClientOutcomeV1 {
    Reply(RemoteEditReplyV1),
    DefinitelyNotSent,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditClientRefusalV1 {
    UncertaintyConflict,
    ReplyMismatch,
}

#[derive(Default)]
pub struct EditAdmissionClientV1 {
    pending: Option<RemoteEditRequestV1>,
}

impl EditAdmissionClientV1 {
    pub fn send<F>(
        &mut self,
        request: RemoteEditRequestV1,
        transmit: F,
    ) -> Result<EditClientOutcomeV1, EditClientRefusalV1>
    where
        F: FnOnce(&RemoteEditRequestV1) -> EditPortOutcomeV1,
    {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending != &request)
        {
            return Err(EditClientRefusalV1::UncertaintyConflict);
        }
        self.pending = Some(request.clone());
        match transmit(&request) {
            EditPortOutcomeV1::Reply { request_id, reply } => {
                if request_id != request.request_id {
                    return Err(EditClientRefusalV1::ReplyMismatch);
                }
                self.pending = None;
                Ok(EditClientOutcomeV1::Reply(reply))
            }
            EditPortOutcomeV1::DefinitelyNotSent => {
                self.pending = None;
                Ok(EditClientOutcomeV1::DefinitelyNotSent)
            }
            EditPortOutcomeV1::OutcomeUnknown => Ok(EditClientOutcomeV1::OutcomeUnknown),
        }
    }

    pub fn pending(&self) -> Option<&RemoteEditRequestV1> {
        self.pending.as_ref()
    }

    pub fn abandon(&mut self) -> Option<RemoteEditRequestV1> {
        self.pending.take()
    }
}
