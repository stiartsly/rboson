use std::result::Result as StdResult;

use log::debug;
use rumqttc::{
    tokio_rustls::rustls::client::danger::{
        HandshakeSignatureValid,
        ServerCertVerified,
        ServerCertVerifier,
    },
    tokio_rustls::rustls::pki_types::{
        CertificateDer,
        ServerName,
        UnixTime,
    },
    tokio_rustls::rustls::{
        DigitallySignedStruct,
        Error as RustlsError,
        SignatureScheme,
    },
};
use boson_api::Id;

#[derive(Debug)]
pub(super) struct BosonServerCertVerifier {
    #[allow(dead_code)]
    expected_peer_id: Option<Id>,
}

impl BosonServerCertVerifier {
    pub(super) fn new(expected_peer_id: Option<Id>) -> Self {
        Self { expected_peer_id }
    }
}

impl ServerCertVerifier for BosonServerCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> StdResult<ServerCertVerified, RustlsError> {
        debug!(
            "TLS: verifying server certificate for {:?}, peer_id={:?}",
            server_name, self.expected_peer_id
        );
        // Accept self-signed / Boson peer certificates bound to the messaging node identity
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> StdResult<HandshakeSignatureValid, RustlsError> {
        rumqttc::tokio_rustls::rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> StdResult<HandshakeSignatureValid, RustlsError> {
        rumqttc::tokio_rustls::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rumqttc::tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
