use vellum_federation_protocol::{
    AuthoritativeWorkspaceEventV1, AuthoritativeWorkspaceSnapshotV1, DisclosureProfileV1,
    EditRefusalClassV1, EditRequestIdV1, FederatedActorRefV1, FederatedDocumentRefV1,
    FederatedWorkspaceRefV1, FederationCapabilitiesV1, FederationHelloV1, FederationLimitsV1,
    FederationSessionEvidenceV1, HomeLocalActorIdV1, HomeLocalDocumentIdV1, HomeLocalWorkspaceIdV1,
    NegotiatedFederationV1, ProfileRangeV1, RemoteEditReplyV1, RemoteEditRequestV1,
    ReplicaAcknowledgementV1, ServiceIncarnationV1, SourceDigestV1, SourceRevisionV1, TrustEpochV1,
    VellumHomeIdV1, VerifiedHomeV1, WorkspaceEventFactV1,
};
use vellum_federation_wire::{
    WireRefusalV1, decode_acknowledgement, decode_edit_reply, decode_edit_request, decode_event,
    decode_hello, decode_session_evidence, decode_snapshot, decode_workspace_event_fact,
    encode_acknowledgement, encode_edit_reply, encode_edit_request, encode_event, encode_hello,
    encode_session_evidence, encode_snapshot, encode_workspace_event_fact,
};

fn workspace() -> FederatedWorkspaceRefV1 {
    FederatedWorkspaceRefV1 {
        home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
        workspace: HomeLocalWorkspaceIdV1::new([0x22; 16]).unwrap(),
    }
}

fn hello() -> FederationHelloV1 {
    FederationHelloV1 {
        home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
        trust_epoch: TrustEpochV1::new(3).unwrap(),
        incarnation: ServiceIncarnationV1::new([0x33; 16]).unwrap(),
        capabilities: FederationCapabilitiesV1 {
            protocol: ProfileRangeV1::new(1, 2).unwrap(),
            disclosure: ProfileRangeV1::new(1, 2).unwrap(),
            event_schema: ProfileRangeV1::new(3, 4).unwrap(),
            snapshot_schema: ProfileRangeV1::new(5, 6).unwrap(),
            edit_schema: ProfileRangeV1::new(7, 8).unwrap(),
            receipt_schema: ProfileRangeV1::new(9, 10).unwrap(),
            limits: FederationLimitsV1 {
                max_frame_bytes: 1024,
                max_source_bytes: 2048,
                max_documents: 11,
                max_in_flight: 12,
                max_subscriptions: 13,
                max_replay_events: 14,
                max_snapshot_bytes: 4096,
                max_receipts: 15,
            },
        },
    }
}

fn edit_request() -> RemoteEditRequestV1 {
    RemoteEditRequestV1::new(
        EditRequestIdV1::new([0x31; 16]).unwrap(),
        FederatedActorRefV1 {
            home: VellumHomeIdV1::new([0x32; 32]).unwrap(),
            actor: HomeLocalActorIdV1::new([0x33; 16]).unwrap(),
        },
        FederatedDocumentRefV1 {
            workspace: FederatedWorkspaceRefV1 {
                home: VellumHomeIdV1::new([0x34; 32]).unwrap(),
                workspace: HomeLocalWorkspaceIdV1::new([0x35; 16]).unwrap(),
            },
            document: HomeLocalDocumentIdV1::new([0x36; 16]).unwrap(),
        },
        SourceRevisionV1::new(7).unwrap(),
        vec![0xde, 0xad, 0xbe, 0xef],
    )
    .unwrap()
}

#[test]
fn edit_request_fixture_is_canonical_bounded_and_opaque() {
    let request = edit_request();
    let frame = encode_edit_request(&request).unwrap();

    assert_eq!(frame.len(), 148);
    assert_eq!(&frame[..4], b"VEQ1");
    assert_eq!(&frame[4..20], &[0x31; 16]);
    assert_eq!(&frame[20..52], &[0x32; 32]);
    assert_eq!(&frame[52..68], &[0x33; 16]);
    assert_eq!(&frame[68..100], &[0x34; 32]);
    assert_eq!(&frame[100..116], &[0x35; 16]);
    assert_eq!(&frame[116..132], &[0x36; 16]);
    assert_eq!(&frame[132..140], &7_u64.to_be_bytes());
    assert_eq!(&frame[140..144], &4_u32.to_be_bytes());
    assert_eq!(&frame[144..], &[0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(decode_edit_request(&frame, 4), Ok(request));
    assert_eq!(
        decode_edit_request(&frame, 3),
        Err(WireRefusalV1::PayloadTooLarge)
    );
    assert_eq!(
        decode_edit_request(&frame, 0),
        Err(WireRefusalV1::InvalidLimits)
    );
}

#[test]
fn edit_request_decoder_refuses_malformed_or_noncanonical_frames() {
    let valid = encode_edit_request(&edit_request()).unwrap();
    assert_eq!(
        decode_edit_request(&valid[..143], 4),
        Err(WireRefusalV1::Truncated)
    );

    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        decode_edit_request(&trailing, 5),
        Err(WireRefusalV1::LengthMismatch)
    );

    let mut declared_too_long = valid.clone();
    declared_too_long[140..144].copy_from_slice(&5_u32.to_be_bytes());
    assert_eq!(
        decode_edit_request(&declared_too_long, 5),
        Err(WireRefusalV1::LengthMismatch)
    );

    for range in [4..20, 20..52, 52..68, 68..100, 100..116, 116..132] {
        let mut zero = valid.clone();
        zero[range].fill(0);
        assert!(matches!(
            decode_edit_request(&zero, 4),
            Err(WireRefusalV1::InvalidRequest | WireRefusalV1::InvalidIdentity)
        ));
    }

    let mut zero_revision = valid.clone();
    zero_revision[132..140].fill(0);
    assert_eq!(
        decode_edit_request(&zero_revision, 4),
        Err(WireRefusalV1::InvalidRequest)
    );

    let mut empty = valid[..144].to_vec();
    empty[140..144].fill(0);
    assert_eq!(
        decode_edit_request(&empty, 4),
        Err(WireRefusalV1::InvalidRequest)
    );
}

#[test]
fn edit_reply_fixtures_are_canonical_and_closed() {
    let request_id = EditRequestIdV1::new([0x31; 16]).unwrap();
    let applied = RemoteEditReplyV1::Applied {
        resulting_revision: SourceRevisionV1::new(8).unwrap(),
    };
    let applied_frame = encode_edit_reply(request_id, applied);
    assert_eq!(applied_frame.len(), 29);
    assert_eq!(&applied_frame[..4], b"VEP1");
    assert_eq!(applied_frame[20], 1);
    assert_eq!(&applied_frame[21..], &8_u64.to_be_bytes());
    assert_eq!(decode_edit_reply(&applied_frame), Ok((request_id, applied)));

    for refusal in [
        EditRefusalClassV1::Unauthorized,
        EditRefusalClassV1::StaleRevision,
        EditRefusalClassV1::InvalidIntent,
        EditRefusalClassV1::Unavailable,
    ] {
        let reply = RemoteEditReplyV1::Refused(refusal);
        let frame = encode_edit_reply(request_id, reply);
        assert_eq!(frame.len(), 22);
        assert_eq!(frame[20], 2);
        assert_eq!(decode_edit_reply(&frame), Ok((request_id, reply)));
    }
}

#[test]
fn edit_reply_decoder_refuses_truncated_trailing_unknown_and_zero_values() {
    let request_id = EditRequestIdV1::new([0x31; 16]).unwrap();
    let mut applied = encode_edit_reply(
        request_id,
        RemoteEditReplyV1::Applied {
            resulting_revision: SourceRevisionV1::new(8).unwrap(),
        },
    );
    assert_eq!(
        decode_edit_reply(&applied[..21]),
        Err(WireRefusalV1::Truncated)
    );
    assert_eq!(
        decode_edit_reply(&applied[..28]),
        Err(WireRefusalV1::Truncated)
    );
    applied.push(0);
    assert_eq!(
        decode_edit_reply(&applied),
        Err(WireRefusalV1::LengthMismatch)
    );

    let mut zero_request = encode_edit_reply(
        request_id,
        RemoteEditReplyV1::Refused(EditRefusalClassV1::Unauthorized),
    );
    zero_request[4..20].fill(0);
    assert_eq!(
        decode_edit_reply(&zero_request),
        Err(WireRefusalV1::InvalidReply)
    );

    let mut unknown_tag = zero_request;
    unknown_tag[4..20].fill(0x31);
    unknown_tag[20] = 99;
    assert_eq!(
        decode_edit_reply(&unknown_tag),
        Err(WireRefusalV1::InvalidReply)
    );

    let mut unknown_refusal = encode_edit_reply(
        request_id,
        RemoteEditReplyV1::Refused(EditRefusalClassV1::Unauthorized),
    );
    unknown_refusal[21] = 99;
    assert_eq!(
        decode_edit_reply(&unknown_refusal),
        Err(WireRefusalV1::InvalidReply)
    );

    let mut zero_revision = encode_edit_reply(
        request_id,
        RemoteEditReplyV1::Applied {
            resulting_revision: SourceRevisionV1::new(8).unwrap(),
        },
    );
    zero_revision[21..29].fill(0);
    assert_eq!(
        decode_edit_reply(&zero_revision),
        Err(WireRefusalV1::InvalidReply)
    );
}

#[test]
fn hello_fixture_binds_disclosure_and_positive_limits() {
    let hello = hello();
    let encoded = encode_hello(hello);
    assert_eq!(encoded.len(), 106);
    assert_eq!(&encoded[..4], b"VFH1");
    assert_eq!(decode_hello(&encoded), Ok(hello));
    assert_eq!(decode_hello(&encoded[..105]), Err(WireRefusalV1::Truncated));
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(decode_hello(&trailing), Err(WireRefusalV1::LengthMismatch));
    let mut invalid_disclosure = encoded;
    invalid_disclosure[64..66].copy_from_slice(&0_u16.to_be_bytes());
    assert_eq!(
        decode_hello(&invalid_disclosure),
        Err(WireRefusalV1::InvalidProfile)
    );
    let mut zero_limit = encode_hello(hello);
    zero_limit[84..88].fill(0);
    assert_eq!(decode_hello(&zero_limit), Err(WireRefusalV1::InvalidLimits));
}

#[test]
fn event_and_snapshot_fixtures_are_canonical() {
    let event = AuthoritativeWorkspaceEventV1::new(
        workspace(),
        7,
        DisclosureProfileV1::Metadata,
        vec![0xaa, 0xbb],
    )
    .unwrap();
    let encoded = encode_event(&event).unwrap();
    let mut expected = b"VFW1".to_vec();
    expected.push(1);
    expected.extend_from_slice(&[0x11; 32]);
    expected.extend_from_slice(&[0x22; 16]);
    expected.extend_from_slice(&1_u16.to_be_bytes());
    expected.extend_from_slice(&7_u64.to_be_bytes());
    expected.extend_from_slice(&2_u32.to_be_bytes());
    expected.extend_from_slice(&[0xaa, 0xbb]);
    assert_eq!(encoded, expected);
    assert_eq!(decode_event(&encoded, 2), Ok(event));

    let snapshot = AuthoritativeWorkspaceSnapshotV1::new(
        workspace(),
        9,
        DisclosureProfileV1::Source,
        vec![0xcc],
    )
    .unwrap();
    assert_eq!(
        decode_snapshot(&encode_snapshot(&snapshot).unwrap(), 1),
        Ok(snapshot)
    );
}

#[test]
fn acknowledgement_fixture_binds_disclosure() {
    let acknowledgement = ReplicaAcknowledgementV1 {
        workspace: workspace(),
        sequence: 12,
    };
    let frame = encode_acknowledgement(acknowledgement, DisclosureProfileV1::Source);
    assert_eq!(
        decode_acknowledgement(&frame),
        Ok((acknowledgement, DisclosureProfileV1::Source))
    );
}

#[test]
fn decoder_refuses_cross_type_trailing_unknown_and_oversize_frames() {
    let event = AuthoritativeWorkspaceEventV1::new(
        workspace(),
        1,
        DisclosureProfileV1::Metadata,
        vec![1, 2],
    )
    .unwrap();
    let frame = encode_event(&event).unwrap();
    assert_eq!(decode_event(&frame, 1), Err(WireRefusalV1::PayloadTooLarge));
    assert_eq!(
        decode_snapshot(&frame, 2),
        Err(WireRefusalV1::WrongFrameType)
    );
    let mut trailing = frame.clone();
    trailing.push(0);
    assert_eq!(
        decode_event(&trailing, 3),
        Err(WireRefusalV1::LengthMismatch)
    );
    let mut unknown_disclosure = frame;
    unknown_disclosure[53..55].copy_from_slice(&99_u16.to_be_bytes());
    assert_eq!(
        decode_event(&unknown_disclosure, 2),
        Err(WireRefusalV1::InvalidDisclosure)
    );
}

#[test]
fn closed_workspace_event_facts_are_canonical_and_content_safe() {
    let document = HomeLocalDocumentIdV1::new([0x44; 16]).unwrap();
    let present = WorkspaceEventFactV1::DocumentPresent {
        document,
        revision: SourceRevisionV1::new(1).unwrap(),
    };
    let advanced = WorkspaceEventFactV1::source_advanced(
        document,
        SourceRevisionV1::new(1).unwrap(),
        SourceRevisionV1::new(2).unwrap(),
        SourceDigestV1::new([0x55; 32]).unwrap(),
    )
    .unwrap();
    let absent = WorkspaceEventFactV1::DocumentAbsent {
        document,
        final_revision: SourceRevisionV1::new(2).unwrap(),
    };

    for fact in [present, advanced, absent] {
        let bytes = encode_workspace_event_fact(fact);
        assert_eq!(decode_workspace_event_fact(&bytes), Ok(fact));
        assert!(!format!("{fact:?}").contains("5555"));
    }

    let encoded = encode_workspace_event_fact(advanced);
    assert_eq!(encoded.len(), 65);
    assert_eq!(encoded[0], 2);
    assert_eq!(&encoded[1..17], &[0x44; 16]);
    assert_eq!(&encoded[17..25], &1_u64.to_be_bytes());
    assert_eq!(&encoded[25..33], &2_u64.to_be_bytes());
    assert_eq!(&encoded[33..], &[0x55; 32]);
}

#[test]
fn workspace_event_fact_decoder_refuses_unknown_noncanonical_and_malformed_values() {
    let document = HomeLocalDocumentIdV1::new([0x44; 16]).unwrap();
    assert!(
        WorkspaceEventFactV1::source_advanced(
            document,
            SourceRevisionV1::new(2).unwrap(),
            SourceRevisionV1::new(2).unwrap(),
            SourceDigestV1::new([0x55; 32]).unwrap(),
        )
        .is_none()
    );

    let valid = encode_workspace_event_fact(WorkspaceEventFactV1::DocumentPresent {
        document,
        revision: SourceRevisionV1::new(1).unwrap(),
    });
    assert_eq!(
        decode_workspace_event_fact(&valid[..24]),
        Err(WireRefusalV1::Truncated)
    );
    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        decode_workspace_event_fact(&trailing),
        Err(WireRefusalV1::LengthMismatch)
    );
    let mut unknown = valid.clone();
    unknown[0] = 99;
    assert_eq!(
        decode_workspace_event_fact(&unknown),
        Err(WireRefusalV1::InvalidEvent)
    );
    let mut zero_document = valid.clone();
    zero_document[1..17].fill(0);
    assert_eq!(
        decode_workspace_event_fact(&zero_document),
        Err(WireRefusalV1::InvalidIdentity)
    );
    let mut zero_revision = valid;
    zero_revision[17..25].fill(0);
    assert_eq!(
        decode_workspace_event_fact(&zero_revision),
        Err(WireRefusalV1::InvalidEvent)
    );
}

fn session_evidence() -> FederationSessionEvidenceV1 {
    FederationSessionEvidenceV1::new(
        VerifiedHomeV1 {
            home: VellumHomeIdV1::new([0x11; 32]).unwrap(),
            trust_epoch: TrustEpochV1::new(3).unwrap(),
        },
        VerifiedHomeV1 {
            home: VellumHomeIdV1::new([0x22; 32]).unwrap(),
            trust_epoch: TrustEpochV1::new(4).unwrap(),
        },
        ServiceIncarnationV1::new([0x33; 16]).unwrap(),
        ServiceIncarnationV1::new([0x44; 16]).unwrap(),
        NegotiatedFederationV1 {
            protocol: 1,
            disclosure: DisclosureProfileV1::Source,
            event_schema: 2,
            snapshot_schema: 3,
            edit_schema: 4,
            receipt_schema: 5,
            limits: FederationLimitsV1 {
                max_frame_bytes: 1_024,
                max_source_bytes: 2_048,
                max_documents: 6,
                max_in_flight: 7,
                max_subscriptions: 8,
                max_replay_events: 9,
                max_snapshot_bytes: 4_096,
                max_receipts: 10,
            },
        },
    )
    .unwrap()
}

#[test]
fn session_evidence_is_canonical_non_authorizing_and_exporter_free() {
    let evidence = session_evidence();
    let bytes = encode_session_evidence(evidence);
    assert_eq!(bytes.len(), 150);
    assert_eq!(&bytes[..4], b"VSE1");
    assert_eq!(decode_session_evidence(&bytes), Ok(evidence));
    assert!(!bytes.windows(32).any(|window| window == [0x99; 32]));
    assert_eq!(
        decode_session_evidence(&bytes[..149]),
        Err(WireRefusalV1::Truncated)
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_session_evidence(&trailing),
        Err(WireRefusalV1::LengthMismatch)
    );
}

#[test]
fn session_evidence_refuses_self_sessions_zero_profiles_and_zero_limits() {
    let mut bytes = encode_session_evidence(session_evidence());
    bytes[44..76].copy_from_slice(&[0x11; 32]);
    assert_eq!(
        decode_session_evidence(&bytes),
        Err(WireRefusalV1::InvalidLimits)
    );
    let mut bytes = encode_session_evidence(session_evidence());
    bytes[116..118].fill(0);
    assert_eq!(
        decode_session_evidence(&bytes),
        Err(WireRefusalV1::InvalidLimits)
    );
    let mut bytes = encode_session_evidence(session_evidence());
    bytes[128..132].fill(0);
    assert_eq!(
        decode_session_evidence(&bytes),
        Err(WireRefusalV1::InvalidLimits)
    );
}
