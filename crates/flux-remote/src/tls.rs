//! TLS configuration: a self-signed server config and a cert-pinning client
//! config that matches the C# `RemoteTcpClient` validation (SHA-256 of the
//! presented certificate must equal the fingerprint embedded in the key).

use crate::protocol;
use anyhow::{Context, Result};
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme};

/// Install the ring crypto provider as process default (idempotent).
pub fn ensure_provider() {
    // Ignore the error: a provider is already installed (e.g. on a second call).
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Build a server config that presents the given self-signed cert + key.
pub fn server_config(cert_der: &[u8], key_der: &[u8]) -> Result<Arc<ServerConfig>> {
    let certs = vec![CertificateDer::from(cert_der.to_vec())];
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der.to_vec()));
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("building server TLS config")?;
    Ok(Arc::new(config))
}

/// Build a client config that pins the server cert to `expected_fingerprint`.
pub fn client_config(expected_fingerprint: [u8; 32]) -> Arc<ClientConfig> {
    let verifier = Arc::new(PinnedVerifier { expected_fingerprint, provider: provider() });
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Arc::new(config)
}

/// The TLS SNI host the client presents — matches C# TargetHost "Flux".
pub fn server_name() -> ServerName<'static> {
    ServerName::try_from("Flux").expect("static name is valid")
}

#[derive(Debug)]
struct PinnedVerifier {
    expected_fingerprint: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let actual = protocol::cert_fingerprint(end_entity.as_ref());
        if protocol::fixed_time_eq(&actual, &self.expected_fingerprint) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate fingerprint mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}
