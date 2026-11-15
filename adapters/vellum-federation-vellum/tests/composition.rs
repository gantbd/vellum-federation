use vellum_federation_edit_port::{AuthoritativeEditPortV1, EditExecutionOutcomeV1};
use vellum_federation_protocol::{
    EditRefusalClassV1, EditRequestIdV1, FederatedActorRefV1, FederatedDocumentRefV1,
    FederatedWorkspaceRefV1, HomeLocalActorIdV1, HomeLocalDocumentIdV1, HomeLocalWorkspaceIdV1,
    RemoteEditReplyV1, RemoteEditRequestV1, SourceRevisionV1, VellumHomeIdV1,
};
use vellum_federation_vellum::{
    ReplacementIntentV1, VellumAuthoritativeEditV1, VellumEditBindingV1,
    encode_replacement_intent_v1,
};
use vellum_protocol::{
    ClientId, DurableEffectIntentV1, DurableEffectReceiptV1, DurableExecuteOutcomeV1,
    DurableExecuteRequestV1, DurableReplaceAndSaveReceiptV1, EditPersistenceV1, RefusalV1,
    SaveApplicationV1, SaveReceiptV1, SaveSynchronizationV1, ServiceAccessErrorV1,
    ServiceControlErrorV1, ServicePort, WorkspaceDocumentIdV1, WorkspaceIdV1,
};

#[derive(Debug)]
struct Service {
    outcome: Result<DurableExecuteOutcomeV1, ServiceAccessErrorV1>,
    seen: Vec<DurableExecuteRequestV1>,
}

impl ServicePort for Service {
    fn connect(&mut self) -> Result<ClientId, ServiceControlErrorV1> {
        panic!("the adapter uses its pre-authenticated client binding")
    }

    fn execute(
        &mut self,
        _: vellum_protocol::RequestV1,
    ) -> Result<vellum_protocol::ReplyV1, ServiceAccessErrorV1> {
        panic!("federated edits use the durable Host path")
    }

    fn disconnect(&mut self, _: ClientId) -> Result<(), ServiceControlErrorV1> {
        Ok(())
    }

    fn execute_durable(
        &mut self,
        request: DurableExecuteRequestV1,
    ) -> Result<DurableExecuteOutcomeV1, ServiceAccessErrorV1> {
        self.seen.push(request);
        self.outcome.clone()
    }
}

fn home(marker: u8) -> VellumHomeIdV1 {
    VellumHomeIdV1::new([marker; 32]).unwrap()
}

fn remote_actor(home_marker: u8, local: u8) -> FederatedActorRefV1 {
    FederatedActorRefV1 {
        home: home(home_marker),
        actor: HomeLocalActorIdV1::new([local; 16]).unwrap(),
    }
}

fn document(home_marker: u8, workspace: u8, document: u8) -> FederatedDocumentRefV1 {
    FederatedDocumentRefV1 {
        workspace: FederatedWorkspaceRefV1 {
            home: home(home_marker),
            workspace: HomeLocalWorkspaceIdV1::new([workspace; 16]).unwrap(),
        },
        document: HomeLocalDocumentIdV1::new([document; 16]).unwrap(),
    }
}

fn request(
    actor: FederatedActorRefV1,
    document: FederatedDocumentRefV1,
    namespace: [u8; 8],
) -> RemoteEditRequestV1 {
    let mut id = [0_u8; 16];
    id[..8].copy_from_slice(&namespace);
    id[15] = 9;
    RemoteEditRequestV1::new(
        EditRequestIdV1::new(id).unwrap(),
        actor,
        document,
        SourceRevisionV1::new(4).unwrap(),
        encode_replacement_intent_v1(
            &ReplacementIntentV1 {
                start: 1,
                end: 3,
                inserted: "hello".into(),
            },
            64,
        )
        .unwrap(),
    )
    .unwrap()
}

fn durable_receipt(
    document: WorkspaceDocumentIdV1,
    persistence: EditPersistenceV1,
) -> DurableExecuteOutcomeV1 {
    DurableExecuteOutcomeV1::Completed(DurableEffectReceiptV1::DocumentReplacedAndSaved(
        DurableReplaceAndSaveReceiptV1 {
            document,
            source_revision: 5,
            text: "redacted from diagnostics".into(),
            persistence,
        },
    ))
}

fn durable() -> EditPersistenceV1 {
    EditPersistenceV1::Durable(SaveReceiptV1 {
        application: SaveApplicationV1::Written,
        synchronization: SaveSynchronizationV1::Succeeded,
        no_future_effects: true,
        quiescent: true,
        adapter_failure: false,
        cleanup_failed: false,
        baseline_published: true,
        outcome_unknown: false,
    })
}

fn composition(
    outcome: Result<DurableExecuteOutcomeV1, ServiceAccessErrorV1>,
) -> (
    VellumAuthoritativeEditV1<Service>,
    FederatedActorRefV1,
    FederatedDocumentRefV1,
    WorkspaceDocumentIdV1,
) {
    let actor = remote_actor(7, 8);
    let document = document(1, 2, 3);
    let workspace = WorkspaceIdV1::new(11).unwrap();
    let local_document = WorkspaceDocumentIdV1::new(workspace, 12).unwrap();
    let binding = VellumEditBindingV1::new(
        actor,
        document,
        ClientId::new(13).unwrap(),
        local_document,
        *b"fed-edit",
        64,
    )
    .unwrap();
    (
        VellumAuthoritativeEditV1::new(
            binding,
            Service {
                outcome,
                seen: Vec::new(),
            },
        ),
        actor,
        document,
        local_document,
    )
}

#[test]
fn exact_binding_uses_durable_host_path_and_returns_only_durable_success() {
    let (_, _, _, local_document) = composition(Err(ServiceAccessErrorV1::Unavailable));
    let (mut adapter, actor, document, _) =
        composition(Ok(durable_receipt(local_document, durable())));
    let request = request(actor, document, *b"fed-edit");
    assert_eq!(
        adapter.execute(&request),
        EditExecutionOutcomeV1::Reply(RemoteEditReplyV1::Applied {
            resulting_revision: SourceRevisionV1::new(5).unwrap(),
        })
    );
    let native = &adapter.service().seen[0];
    assert_eq!(native.operation.to_bytes(), request.request_id.as_bytes());
    assert_eq!(native.request.get(), 9);
    match &native.intent {
        DurableEffectIntentV1::ReplaceDocumentAndSave {
            document,
            expected_revision,
            start,
            end,
            inserted,
        } => {
            assert_eq!(*document, local_document);
            assert_eq!(*expected_revision, 4);
            assert_eq!((*start, *end, inserted.as_str()), (1, 3, "hello"));
        }
        _ => panic!("unexpected native intent"),
    }
}

#[test]
fn binding_intent_namespace_and_close_refuse_before_host_execution() {
    let (mut adapter, actor, document, _) = composition(Ok(DurableExecuteOutcomeV1::Refused(
        RefusalV1::AuthorityDenied,
    )));
    let wrong_actor = remote_actor(7, 9);
    assert_eq!(
        adapter.execute(&request(wrong_actor, document, *b"fed-edit")),
        EditExecutionOutcomeV1::Reply(RemoteEditReplyV1::Refused(EditRefusalClassV1::Unauthorized))
    );
    assert_eq!(
        adapter.execute(&request(actor, document, *b"bad-name")),
        EditExecutionOutcomeV1::Reply(RemoteEditReplyV1::Refused(
            EditRefusalClassV1::InvalidIntent
        ))
    );
    adapter.close();
    assert_eq!(
        adapter.execute(&request(actor, document, *b"fed-edit")),
        EditExecutionOutcomeV1::Reply(RemoteEditReplyV1::Refused(EditRefusalClassV1::Unavailable))
    );
    assert!(adapter.service().seen.is_empty());
}

#[test]
fn uncertainty_and_non_durable_completion_never_become_stable_success() {
    let (_, _, _, local_document) = composition(Err(ServiceAccessErrorV1::Unavailable));
    let not_durable = EditPersistenceV1::NotAttempted(RefusalV1::AuthorityDenied);
    for (outcome, expected) in [
        (
            Err(ServiceAccessErrorV1::NotSent),
            EditExecutionOutcomeV1::DefinitelyNotExecuted,
        ),
        (
            Err(ServiceAccessErrorV1::OutcomeUnknown),
            EditExecutionOutcomeV1::OutcomeUnknown,
        ),
        (
            Ok(durable_receipt(local_document, not_durable)),
            EditExecutionOutcomeV1::OutcomeUnknown,
        ),
        (
            Ok(DurableExecuteOutcomeV1::Retired),
            EditExecutionOutcomeV1::OutcomeUnknown,
        ),
    ] {
        let (mut adapter, actor, document, _) = composition(outcome);
        assert_eq!(
            adapter.execute(&request(actor, document, *b"fed-edit")),
            expected
        );
    }
}

#[test]
fn diagnostics_do_not_disclose_inserted_text_or_native_client() {
    let (adapter, _, _, _) = composition(Err(ServiceAccessErrorV1::Unavailable));
    let diagnostic = format!("{adapter:?}");
    assert!(!diagnostic.contains("redacted from diagnostics"));
    assert!(!diagnostic.contains("ClientId"));
}
