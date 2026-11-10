//! TLS 1.3 for QUIC: throwaway self-signed certificates and verifiers that
//! accept any certificate but check the handshake signature.
//!
//! Certificates are not trusted by themselves: the PAKE key confirmation
//! binds both certificate hashes and the TLS exporter, so a substituted
//! certificate fails pairing. The verifiers still verify the TLS 1.3
//! `CertificateVerify` signature properly, so the peer provably holds the
//! private key of the certificate it presented.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};

use crate::ALPN;
use crate::error::{ProtocolViolation, Result, TransportError};

/// The crypto provider: ring, default cipher order (AES-128-GCM first).
fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// This process's certificate and key for one listener or connection.
pub(crate) struct Identity {
    cert: CertificateDer<'static>,
    key: PrivatePkcs8KeyDer<'static>,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Identity(<redacted>)")
    }
}

impl Identity {
    /// Fresh self-signed certificate (ECDSA P-256).
    pub(crate) fn generate() -> Result<Self> {
        let certified =
            rcgen::generate_simple_self_signed(vec![crate::tuning::TLS_SERVER_NAME.to_owned()])
                .map_err(|e| TransportError::internal("certificate generation", e))?;
        Ok(Self {
            cert: certified.cert.der().clone(),
            key: PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der()),
        })
    }

    /// DER certificate (hashed into the channel binding).
    pub(crate) fn cert_der(&self) -> &[u8] {
        &self.cert
    }

    fn chain(&self) -> Vec<CertificateDer<'static>> {
        vec![self.cert.clone()]
    }

    fn key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(self.key.clone_key())
    }

    /// Server (sender) TLS config: TLS 1.3 only, ALPN, client certificate
    /// required.
    pub(crate) fn server_config(&self) -> Result<rustls::ServerConfig> {
        let provider = provider();
        let verifier = Arc::new(AnyCert::new(&provider));
        let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| TransportError::internal("TLS server config", e))?
            .with_client_cert_verifier(verifier)
            .with_single_cert(self.chain(), self.key())
            .map_err(|e| TransportError::internal("TLS server certificate", e))?;
        cfg.alpn_protocols = vec![ALPN.to_vec()];
        Ok(cfg)
    }

    /// Client (receiver) TLS config: TLS 1.3 only, ALPN, client
    /// certificate presented.
    pub(crate) fn client_config(&self) -> Result<rustls::ClientConfig> {
        let provider = provider();
        let verifier = Arc::new(AnyCert::new(&provider));
        let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| TransportError::internal("TLS client config", e))?
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_client_auth_cert(self.chain(), self.key())
            .map_err(|e| TransportError::internal("TLS client certificate", e))?;
        cfg.alpn_protocols = vec![ALPN.to_vec()];
        Ok(cfg)
    }
}

/// Accepts any end-entity certificate (authentication happens in the
/// PAKE), verifies handshake signatures with the provider's algorithms.
#[derive(Debug)]
struct AnyCert {
    algorithms: WebPkiSupportedAlgorithms,
}

impl AnyCert {
    fn new(provider: &CryptoProvider) -> Self {
        Self {
            algorithms: provider.signature_verification_algorithms,
        }
    }
}

impl ServerCertVerifier for AnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

impl ClientCertVerifier for AnyCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

/// The peer's end-entity certificate as presented in the TLS handshake.
pub(crate) fn peer_cert(conn: &quinn::Connection) -> Result<CertificateDer<'static>> {
    conn.peer_identity()
        .and_then(|id| id.downcast::<Vec<CertificateDer<'static>>>().ok())
        .and_then(|chain| chain.into_iter().next())
        .ok_or_else(|| TransportError::from(ProtocolViolation::NoPeerCertificate))
}
