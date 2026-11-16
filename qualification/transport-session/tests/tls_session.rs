use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;
use vellum_federation_configured_trust::{
    ConfiguredTrustContextV1, ConfiguredTrustV1, PeerProvenanceV1,
};
use vellum_federation_connection::{FederationConnectionRefusalV1, FederationConnectionV1};
use vellum_federation_protocol::{
    DisclosureProfileV1, FederationCapabilitiesV1, FederationHelloV1, FederationLimitsV1,
    ProfileRangeV1, ServiceIncarnationV1, TrustEpochV1, VellumHomeIdV1, VerifiedHomeV1,
};
use vellum_federation_rustls_transport::{
    PeerTransportProvenanceV1, accept_v1, client_config_v1, connect_v1, server_config_v1,
};
use vellum_federation_session::establish_session_v1;
use vellum_federation_wire::{decode_hello, encode_hello};

const TIMEOUT: Duration = Duration::from_secs(2);

fn hello(marker: u8, epoch: u64, incarnation: u8) -> FederationHelloV1 {
    let profile = ProfileRangeV1::new(1, 1).unwrap();
    let source = ProfileRangeV1::new(2, 2).unwrap();
    FederationHelloV1 {
        home: VellumHomeIdV1::new([marker; 32]).unwrap(),
        trust_epoch: TrustEpochV1::new(epoch).unwrap(),
        incarnation: ServiceIncarnationV1::new([incarnation; 16]).unwrap(),
        capabilities: FederationCapabilitiesV1 {
            protocol: profile,
            disclosure: source,
            event_schema: profile,
            snapshot_schema: profile,
            edit_schema: profile,
            receipt_schema: profile,
            limits: FederationLimitsV1 {
                max_frame_bytes: 1_024,
                max_source_bytes: 2_048,
                max_documents: 4,
                max_in_flight: 4,
                max_subscriptions: 4,
                max_replay_events: 8,
                max_snapshot_bytes: 4_096,
                max_receipts: 8,
            },
        },
    }
}

fn verified(hello: FederationHelloV1) -> VerifiedHomeV1 {
    VerifiedHomeV1 {
        home: hello.home,
        trust_epoch: hello.trust_epoch,
    }
}

#[test]
fn mutual_tls_exporter_and_live_trust_revocation_gate_every_transport_use() {
    qualify_revocation(
        DisclosureProfileV1::Source,
        PeerProvenanceV1::EncryptedChannel,
    );
}

#[test]
fn live_disclosure_downgrade_gates_every_transport_use() {
    qualify_revocation(
        DisclosureProfileV1::Metadata,
        PeerProvenanceV1::VerifiedCertificate,
    );
}

fn qualify_revocation(
    revoked_disclosure: DisclosureProfileV1,
    revoked_provenance: PeerProvenanceV1,
) {
    let server_hello = hello(1, 3, 5);
    let client_hello = hello(2, 4, 6);
    let server_config =
        Arc::new(server_config_v1(vec![server_cert()], server_key(), vec![ca_cert()]).unwrap());
    let client_config =
        Arc::new(client_config_v1(vec![ca_cert()], vec![client_cert()], client_key()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (exporter_tx, exporter_rx) = mpsc::channel();

    let server = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        let mut transport = accept_v1(socket, server_config, TIMEOUT).unwrap();
        assert_eq!(
            transport.peer_provenance(),
            PeerTransportProvenanceV1::VerifiedCertificate
        );
        transport.write_all(&encode_hello(server_hello)).unwrap();
        let mut peer_hello = [0; 106];
        transport.read_exact(&mut peer_hello).unwrap();
        let client_hello = decode_hello(&peer_hello).unwrap();
        let exporter = transport.exporter();
        let context = ConfiguredTrustContextV1::new(
            verified(server_hello),
            verified(client_hello),
            exporter,
            DisclosureProfileV1::Source,
            PeerProvenanceV1::VerifiedCertificate,
        );
        establish_session_v1(
            &mut ConfiguredTrustV1,
            &context,
            server_hello,
            client_hello,
            exporter,
        )
        .unwrap();
        exporter_tx.send(exporter).unwrap();
        let mut byte = [0; 1];
        transport.read_exact(&mut byte).unwrap();
        assert_eq!(byte, [7]);
        assert_eq!(transport.read(&mut byte).unwrap(), 0);
    });

    let mut transport = connect_v1(
        TcpStream::connect(address).unwrap(),
        client_config,
        "localhost",
        TIMEOUT,
    )
    .unwrap();
    let mut peer_hello = [0; 106];
    transport.read_exact(&mut peer_hello).unwrap();
    let server_hello = decode_hello(&peer_hello).unwrap();
    transport.write_all(&encode_hello(client_hello)).unwrap();
    let exporter = transport.exporter();
    assert_eq!(exporter_rx.recv().unwrap(), exporter);
    let current = ConfiguredTrustContextV1::new(
        verified(client_hello),
        verified(server_hello),
        exporter,
        DisclosureProfileV1::Source,
        PeerProvenanceV1::VerifiedCertificate,
    );
    let session = establish_session_v1(
        &mut ConfiguredTrustV1,
        &current,
        client_hello,
        server_hello,
        exporter,
    )
    .unwrap();
    let mut connection = FederationConnectionV1::new(session, transport);
    connection
        .revalidate_and_use(&mut ConfiguredTrustV1, &current, |transport| {
            transport.write_all(&[7])
        })
        .unwrap()
        .unwrap();

    let revoked = ConfiguredTrustContextV1::new(
        verified(client_hello),
        verified(server_hello),
        exporter,
        revoked_disclosure,
        revoked_provenance,
    );
    let mut used_after_revocation = false;
    assert_eq!(
        connection.revalidate_and_use(&mut ConfiguredTrustV1, &revoked, |_| {
            used_after_revocation = true;
        }),
        Err(FederationConnectionRefusalV1::TrustOrDisclosureRevoked)
    );
    assert!(!used_after_revocation);
    server.join().unwrap();
}

fn ca_cert() -> Vec<u8> {
    decode_hex(include_str!("fixtures/ca.der.hex"))
}

fn server_cert() -> Vec<u8> {
    decode_hex(include_str!("fixtures/server.der.hex"))
}

fn server_key() -> Vec<u8> {
    decode_hex(include_str!("fixtures/server-key.der.hex"))
}

fn client_cert() -> Vec<u8> {
    decode_hex(include_str!("fixtures/client.der.hex"))
}

fn client_key() -> Vec<u8> {
    decode_hex(include_str!("fixtures/client-key.der.hex"))
}

fn decode_hex(input: &str) -> Vec<u8> {
    let compact = input.split_whitespace().collect::<String>();
    assert_eq!(compact.len() % 2, 0);
    (0..compact.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&compact[offset..offset + 2], 16).unwrap())
        .collect()
}
