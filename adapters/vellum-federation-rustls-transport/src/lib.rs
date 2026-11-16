//! TLS 1.3 physical transport with Vellum-specific ALPN and exporter binding.

#![forbid(unsafe_code)]

use rustls::client::Resumption;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::server::WebPkiClientVerifier;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
    version,
};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;
use vellum_federation_protocol::TransportExporterV1;
use vellum_federation_transport_port::FederationTransportLifecyclePortV1;

pub const ALPN_V1: &[u8] = b"vellum-federation/1";
const EXPORTER_LABEL_V1: &[u8] = b"EXPORTER-Vellum-Federation-Channel-Binding-V1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportTlsErrorV1 {
    InvalidCertificate,
    InvalidPrivateKey,
    EmptyTrustStore,
    InvalidServerName,
    Configuration,
    Io,
    Handshake,
    Alpn,
    Protocol,
    Exporter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerTransportProvenanceV1 {
    EncryptedChannel,
    VerifiedCertificate,
}

pub fn server_config_v1(
    certificate_chain: Vec<Vec<u8>>,
    private_key_pkcs8: Vec<u8>,
    required_client_roots: Vec<Vec<u8>>,
) -> Result<ServerConfig, TransportTlsErrorV1> {
    let certificates = certificates(certificate_chain)?;
    let key = private_key(private_key_pkcs8)?;
    let verifier = WebPkiClientVerifier::builder(Arc::new(root_store(required_client_roots)?))
        .build()
        .map_err(|_| TransportTlsErrorV1::Configuration)?;
    let mut config = ServerConfig::builder_with_protocol_versions(&[&version::TLS13])
        .with_client_cert_verifier(verifier)
        .with_single_cert(certificates, key)
        .map_err(|_| TransportTlsErrorV1::Configuration)?;
    config.alpn_protocols = vec![ALPN_V1.to_vec()];
    config.max_early_data_size = 0;
    config.send_half_rtt_data = false;
    config.send_tls13_tickets = 0;
    Ok(config)
}

pub fn client_config_v1(
    server_roots: Vec<Vec<u8>>,
    certificate_chain: Vec<Vec<u8>>,
    private_key_pkcs8: Vec<u8>,
) -> Result<ClientConfig, TransportTlsErrorV1> {
    let mut config = ClientConfig::builder_with_protocol_versions(&[&version::TLS13])
        .with_root_certificates(root_store(server_roots)?)
        .with_client_auth_cert(
            certificates(certificate_chain)?,
            private_key(private_key_pkcs8)?,
        )
        .map_err(|_| TransportTlsErrorV1::Configuration)?;
    config.alpn_protocols = vec![ALPN_V1.to_vec()];
    config.enable_early_data = false;
    config.resumption = Resumption::disabled();
    Ok(config)
}

pub struct ServerTransportV1 {
    stream: StreamOwned<ServerConnection, TcpStream>,
    exporter: TransportExporterV1,
    provenance: PeerTransportProvenanceV1,
    closed: bool,
}

pub struct ClientTransportV1 {
    stream: StreamOwned<ClientConnection, TcpStream>,
    exporter: TransportExporterV1,
    closed: bool,
}

impl ServerTransportV1 {
    pub const fn exporter(&self) -> TransportExporterV1 {
        self.exporter
    }

    pub const fn peer_provenance(&self) -> PeerTransportProvenanceV1 {
        self.provenance
    }
}

impl ClientTransportV1 {
    pub const fn exporter(&self) -> TransportExporterV1 {
        self.exporter
    }
}

pub fn accept_v1(
    mut socket: TcpStream,
    config: Arc<ServerConfig>,
    timeout: Duration,
) -> Result<ServerTransportV1, TransportTlsErrorV1> {
    set_timeout(&socket, timeout)?;
    let mut connection =
        ServerConnection::new(config).map_err(|_| TransportTlsErrorV1::Configuration)?;
    complete_server_handshake(&mut connection, &mut socket)?;
    validate(connection.protocol_version(), connection.alpn_protocol())?;
    let provenance = if connection
        .peer_certificates()
        .is_some_and(|certificates| !certificates.is_empty())
    {
        PeerTransportProvenanceV1::VerifiedCertificate
    } else {
        PeerTransportProvenanceV1::EncryptedChannel
    };
    let exporter = export_server_binding(&connection)?;
    Ok(ServerTransportV1 {
        stream: StreamOwned::new(connection, socket),
        exporter,
        provenance,
        closed: false,
    })
}

pub fn connect_v1(
    mut socket: TcpStream,
    config: Arc<ClientConfig>,
    server_name: &str,
    timeout: Duration,
) -> Result<ClientTransportV1, TransportTlsErrorV1> {
    set_timeout(&socket, timeout)?;
    let server_name = ServerName::try_from(server_name.to_owned())
        .map_err(|_| TransportTlsErrorV1::InvalidServerName)?;
    let mut connection = ClientConnection::new(config, server_name)
        .map_err(|_| TransportTlsErrorV1::Configuration)?;
    complete_client_handshake(&mut connection, &mut socket)?;
    validate(connection.protocol_version(), connection.alpn_protocol())?;
    let exporter = export_client_binding(&connection)?;
    Ok(ClientTransportV1 {
        stream: StreamOwned::new(connection, socket),
        exporter,
        closed: false,
    })
}

macro_rules! stream_io {
    ($transport:ty) => {
        impl Read for $transport {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                self.stream.read(buffer)
            }
        }

        impl Write for $transport {
            fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
                self.stream.write(buffer)
            }

            fn flush(&mut self) -> io::Result<()> {
                self.stream.flush()
            }
        }

        impl FederationTransportLifecyclePortV1 for $transport {
            type Error = io::Error;

            fn close(&mut self) -> Result<(), Self::Error> {
                if self.closed {
                    return Ok(());
                }
                self.closed = true;
                self.stream.conn.send_close_notify();
                self.stream.flush()
            }
        }
    };
}

stream_io!(ServerTransportV1);
stream_io!(ClientTransportV1);

fn certificates(values: Vec<Vec<u8>>) -> Result<Vec<CertificateDer<'static>>, TransportTlsErrorV1> {
    if values.is_empty() || values.iter().any(Vec::is_empty) {
        return Err(TransportTlsErrorV1::InvalidCertificate);
    }
    Ok(values.into_iter().map(CertificateDer::from).collect())
}

fn private_key(value: Vec<u8>) -> Result<PrivateKeyDer<'static>, TransportTlsErrorV1> {
    if value.is_empty() {
        return Err(TransportTlsErrorV1::InvalidPrivateKey);
    }
    Ok(PrivateKeyDer::from(PrivatePkcs8KeyDer::from(value)))
}

fn root_store(values: Vec<Vec<u8>>) -> Result<RootCertStore, TransportTlsErrorV1> {
    if values.is_empty() {
        return Err(TransportTlsErrorV1::EmptyTrustStore);
    }
    let mut roots = RootCertStore::empty();
    for value in values {
        roots
            .add(CertificateDer::from(value))
            .map_err(|_| TransportTlsErrorV1::InvalidCertificate)?;
    }
    Ok(roots)
}

fn set_timeout(socket: &TcpStream, timeout: Duration) -> Result<(), TransportTlsErrorV1> {
    if timeout.is_zero() {
        return Err(TransportTlsErrorV1::Configuration);
    }
    socket
        .set_read_timeout(Some(timeout))
        .and_then(|()| socket.set_write_timeout(Some(timeout)))
        .map_err(|_| TransportTlsErrorV1::Io)
}

fn complete_server_handshake(
    connection: &mut ServerConnection,
    socket: &mut TcpStream,
) -> Result<(), TransportTlsErrorV1> {
    while connection.is_handshaking() {
        connection
            .complete_io(socket)
            .map_err(|_| TransportTlsErrorV1::Handshake)?;
    }
    Ok(())
}

fn complete_client_handshake(
    connection: &mut ClientConnection,
    socket: &mut TcpStream,
) -> Result<(), TransportTlsErrorV1> {
    while connection.is_handshaking() {
        connection
            .complete_io(socket)
            .map_err(|_| TransportTlsErrorV1::Handshake)?;
    }
    Ok(())
}

fn validate(
    version: Option<rustls::ProtocolVersion>,
    alpn: Option<&[u8]>,
) -> Result<(), TransportTlsErrorV1> {
    if version != Some(rustls::ProtocolVersion::TLSv1_3) {
        return Err(TransportTlsErrorV1::Protocol);
    }
    if alpn != Some(ALPN_V1) {
        return Err(TransportTlsErrorV1::Alpn);
    }
    Ok(())
}

fn export_server_binding(
    connection: &ServerConnection,
) -> Result<TransportExporterV1, TransportTlsErrorV1> {
    let mut bytes = [0; 32];
    connection
        .export_keying_material(&mut bytes, EXPORTER_LABEL_V1, None)
        .map_err(|_| TransportTlsErrorV1::Exporter)?;
    TransportExporterV1::new(bytes).ok_or(TransportTlsErrorV1::Exporter)
}

fn export_client_binding(
    connection: &ClientConnection,
) -> Result<TransportExporterV1, TransportTlsErrorV1> {
    let mut bytes = [0; 32];
    connection
        .export_keying_material(&mut bytes, EXPORTER_LABEL_V1, None)
        .map_err(|_| TransportTlsErrorV1::Exporter)?;
    TransportExporterV1::new(bytes).ok_or(TransportTlsErrorV1::Exporter)
}
