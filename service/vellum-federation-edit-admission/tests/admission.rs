use vellum_federation_edit_admission::{
    EditAdmissionApplyV1, EditAdmissionClientV1, EditAdmissionHomeV1, EditAdmissionLimitsV1,
    EditAdmissionRefusalV1, EditClientOutcomeV1, EditClientRefusalV1, EditPortOutcomeV1,
};
use vellum_federation_protocol::{
    EditRefusalClassV1, EditRequestIdV1, FederatedActorRefV1, FederatedDocumentRefV1,
    FederatedWorkspaceRefV1, HomeLocalActorIdV1, HomeLocalDocumentIdV1, HomeLocalWorkspaceIdV1,
    RemoteEditReplyV1, RemoteEditRequestV1, SourceRevisionV1, VellumHomeIdV1,
};

fn home(value: u8) -> VellumHomeIdV1 {
    VellumHomeIdV1::new([value; 32]).unwrap()
}

fn request(id: u8, sender: u8, workspace_home: u8, intent: u8) -> RemoteEditRequestV1 {
    RemoteEditRequestV1::new(
        EditRequestIdV1::new([id; 16]).unwrap(),
        FederatedActorRefV1 {
            home: home(sender),
            actor: HomeLocalActorIdV1::new([sender; 16]).unwrap(),
        },
        FederatedDocumentRefV1 {
            workspace: FederatedWorkspaceRefV1 {
                home: home(workspace_home),
                workspace: HomeLocalWorkspaceIdV1::new([7; 16]).unwrap(),
            },
            document: HomeLocalDocumentIdV1::new([8; 16]).unwrap(),
        },
        SourceRevisionV1::new(4).unwrap(),
        vec![intent],
    )
    .unwrap()
}

fn applied(revision: u64) -> RemoteEditReplyV1 {
    RemoteEditReplyV1::Applied {
        resulting_revision: SourceRevisionV1::new(revision).unwrap(),
    }
}

fn limits() -> EditAdmissionLimitsV1 {
    EditAdmissionLimitsV1 {
        max_receipts: 2,
        max_intent_bytes: 4,
    }
}

#[test]
fn home_preflights_scope_and_replays_without_second_execution() {
    let mut admission = EditAdmissionHomeV1::new(home(1), limits()).unwrap();
    let first = request(9, 2, 1, 3);
    let mut executions = 0;
    assert_eq!(
        admission.admit(home(2), first.clone(), |_| {
            executions += 1;
            applied(5)
        }),
        Ok(EditAdmissionApplyV1::Applied(applied(5)))
    );
    assert_eq!(
        admission.admit(home(2), first.clone(), |_| {
            executions += 1;
            applied(6)
        }),
        Ok(EditAdmissionApplyV1::Replay(applied(5)))
    );
    assert_eq!(executions, 1);
    assert_eq!(admission.receipt_count(), 1);
    assert_eq!(
        admission.admit(home(2), request(9, 2, 1, 4), |_| applied(5)),
        Err(EditAdmissionRefusalV1::RequestConflict)
    );
    assert_eq!(
        admission.admit(home(3), request(10, 2, 1, 3), |_| applied(5)),
        Err(EditAdmissionRefusalV1::ActorSourceMismatch)
    );
    assert_eq!(
        admission.admit(home(2), request(10, 2, 3, 3), |_| applied(5)),
        Err(EditAdmissionRefusalV1::NotWorkspaceHome)
    );
}

#[test]
fn replies_are_revision_safe_and_receipts_recover_exactly() {
    let mut admission = EditAdmissionHomeV1::new(home(1), limits()).unwrap();
    assert_eq!(
        admission.admit(home(2), request(9, 2, 1, 3), |_| applied(4)),
        Err(EditAdmissionRefusalV1::InvalidReply)
    );
    let refused = RemoteEditReplyV1::Refused(EditRefusalClassV1::Unauthorized);
    assert_eq!(
        admission.admit(home(2), request(9, 2, 1, 3), |_| refused),
        Ok(EditAdmissionApplyV1::Applied(refused))
    );
    let checkpoint = admission.checkpoint();
    let mut recovered = EditAdmissionHomeV1::recover(checkpoint, limits()).unwrap();
    assert_eq!(
        recovered.admit(home(2), request(9, 2, 1, 3), |_| applied(5)),
        Ok(EditAdmissionApplyV1::Replay(refused))
    );
}

#[test]
fn client_retains_only_exact_uncertainty_and_clears_proven_absence_or_reply() {
    let mut client = EditAdmissionClientV1::default();
    let first = request(9, 2, 1, 3);
    assert_eq!(
        client.send(first.clone(), |_| EditPortOutcomeV1::OutcomeUnknown),
        Ok(EditClientOutcomeV1::OutcomeUnknown)
    );
    assert_eq!(client.pending(), Some(&first));
    assert_eq!(
        client.send(request(10, 2, 1, 3), |_| {
            EditPortOutcomeV1::DefinitelyNotSent
        }),
        Err(EditClientRefusalV1::UncertaintyConflict)
    );
    assert_eq!(
        client.send(first.clone(), |_| EditPortOutcomeV1::DefinitelyNotSent),
        Ok(EditClientOutcomeV1::DefinitelyNotSent)
    );
    assert!(client.pending().is_none());
    assert_eq!(
        client.send(first.clone(), |_| EditPortOutcomeV1::Reply {
            request_id: first.request_id,
            reply: applied(5),
        }),
        Ok(EditClientOutcomeV1::Reply(applied(5)))
    );
    assert!(client.pending().is_none());
}

#[test]
fn request_debug_redacts_intent_and_actor_kind_never_enters_policy() {
    let request = RemoteEditRequestV1::new(
        EditRequestIdV1::new([9; 16]).unwrap(),
        FederatedActorRefV1 {
            home: home(2),
            actor: HomeLocalActorIdV1::new([3; 16]).unwrap(),
        },
        request(1, 2, 1, 3).document,
        SourceRevisionV1::new(4).unwrap(),
        b"human-agent-neutral-secret".to_vec(),
    )
    .unwrap();
    let debug = format!("{request:?}");
    assert!(!debug.contains("human-agent-neutral-secret"));
    assert!(debug.contains("intent_bytes"));
}
