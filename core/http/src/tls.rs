// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! TLS trust for Boson clients, as the Java SDK's `HybridTrustManager`.
//!
//! A super node may serve a certificate signed by a public CA, or a self-signed one. A CA-signed
//! chain is verified as usual, by the platform's verifier. A self-signed certificate is trusted only
//! when it proves the expected Boson identity:
//!
//! - **Identity binding** (current): a browser-compatible certificate (typically ECDSA P-256) whose
//!   `issuerAltName` carries `boson:certbind:1:<base58 public key>:<base64url signature>`, an Ed25519
//!   signature by the expected identity over `boson:certbind:1` followed by the certificate's
//!   SubjectPublicKeyInfo. The first form, `boson:ed25519:<public key>:<signature>` over the bare
//!   SubjectPublicKeyInfo, counts only in a certificate that carries no current one.
//! - **Ed25519 certificate** (legacy, no binding): its Common Name must be the expected id in Base58
//!   and its raw public key the expected key.
//!
//! [`client_config`] gives a ready `rustls::ClientConfig` for HTTP clients such as reqwest.

use std::sync::Arc;

use base64::{
    Engine as _,
    alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use boson_api::{Id, signature};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, aws_lc_rs, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use x509_parser::{
    certificate::X509Certificate,
    extensions::{GeneralName, ParsedExtension},
    prelude::FromDer,
};

/// What a current identity binding's signature covers ahead of the SubjectPublicKeyInfo.
pub const CERT_BINDING_LABEL: &str = "boson:certbind:1";

const CERT_BINDING_NAMESPACE_PREFIX: &str = "boson:certbind:";
const LEGACY_BINDING_PREFIX: &str = "boson:ed25519:";

// Java's Base64.getUrlDecoder(): the URL-safe alphabet, padding optional.
const BASE64_URL: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// A parsed Boson identity binding: the endorsing Ed25519 public key and its signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityBinding {
    /// The Ed25519 public key that endorses the certificate.
    pub public_key: Vec<u8>,
    /// Its signature over [`IdentityBinding::signed_message`].
    pub signature: Vec<u8>,
    /// Whether this is the first form, whose signature covers the bare SubjectPublicKeyInfo.
    pub legacy: bool,
}

impl IdentityBinding {
    /// What the signature covers, for a certificate with this SubjectPublicKeyInfo DER.
    pub fn signed_message(&self, spki: &[u8]) -> Vec<u8> {
        if self.legacy { spki.to_vec() } else { cert_binding_message(spki) }
    }
}

/// What a current binding's signature covers: [`CERT_BINDING_LABEL`], then the certificate's
/// SubjectPublicKeyInfo DER.
pub fn cert_binding_message(spki: &[u8]) -> Vec<u8> {
    [CERT_BINDING_LABEL.as_bytes(), spki].concat()
}

/// Parses a Boson identity binding URI, in either form. `Ok(None)` when `uri` is no binding at all;
/// an error when it has a binding's prefix but is malformed.
pub fn parse_identity_binding(uri: &str) -> Result<Option<IdentityBinding>, String> {
    if uri.starts_with(CERT_BINDING_NAMESPACE_PREFIX) {
        let fields = boson_string_fields(uri, &format!("{CERT_BINDING_LABEL}:"), 2)
            .ok_or("Malformed Boson certificate binding URI")?;
        return decode_binding(fields[0], fields[1], false).map(Some);
    }
    let Some(rest) = uri.strip_prefix(LEGACY_BINDING_PREFIX) else {
        return Ok(None);
    };
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() != 2 {
        return Err("Malformed Boson identity binding URI".into());
    }
    decode_binding(parts[0], parts[1], true).map(Some)
}

// The fields of a Boson string with the given head, as the Java SDK's BosonString.parse reads them:
// split with a limit, every field non-empty printable ASCII without '?' or '#', ':' only in the last.
fn boson_string_fields<'a>(text: &'a str, head: &str, count: usize) -> Option<Vec<&'a str>> {
    let fields: Vec<&str> = text.trim().strip_prefix(head)?.splitn(count, ':').collect();
    let valid = fields.len() == count
        && fields.iter().enumerate().all(|(i, field)| {
            !field.is_empty()
                && field.chars().all(|c| {
                    ('\x21'..='\x7e').contains(&c) && c != '?' && c != '#' && (c != ':' || i == count - 1)
                })
        });
    valid.then_some(fields)
}

fn decode_binding(public_key: &str, signature: &str, legacy: bool) -> Result<IdentityBinding, String> {
    Ok(IdentityBinding {
        public_key: bs58::decode(public_key)
            .into_vec()
            .map_err(|e| format!("Malformed binding public key: {e}"))?,
        signature: BASE64_URL
            .decode(signature)
            .map_err(|e| format!("Malformed binding signature: {e}"))?,
        legacy,
    })
}

/// A rustls certificate verifier that trusts CA-signed chains as the platform does, and a self-signed
/// certificate only when it proves the expected Boson identity (see the module documentation).
#[derive(Debug)]
pub struct HybridTrustManager {
    expected_cn: String,
    expected_public_key: Vec<u8>,
    default: Arc<dyn ServerCertVerifier>,
    provider: Arc<CryptoProvider>,
}

impl HybridTrustManager {
    /// Trusts CA-signed chains through the platform's verifier, and self-signed certificates proving
    /// `expected`.
    pub fn new(expected: &Id, provider: Arc<CryptoProvider>) -> Result<Self, Error> {
        let default = Arc::new(rustls_platform_verifier::Verifier::new(provider.clone())?);
        Ok(Self::with_default_verifier(expected, default, provider))
    }

    /// Trusts CA-signed chains through `default` instead of the platform's verifier, and self-signed
    /// certificates proving `expected`.
    pub fn with_default_verifier(
        expected: &Id,
        default: Arc<dyn ServerCertVerifier>,
        provider: Arc<CryptoProvider>,
    ) -> Self {
        Self {
            expected_cn: expected.to_base58(),
            expected_public_key: expected.as_bytes().to_vec(),
            default,
            provider,
        }
    }

    fn check_self_signed(&self, cert: &X509Certificate<'_>, now: UnixTime) -> Result<(), Error> {
        // 1. validity
        let now = now.as_secs() as i64;
        let validity = cert.validity();
        if now < validity.not_before.timestamp() {
            return Err(Error::InvalidCertificate(CertificateError::NotValidYet));
        }
        if now > validity.not_after.timestamp() {
            return Err(Error::InvalidCertificate(CertificateError::Expired));
        }

        // 2. the self-signature
        cert.verify_signature(None)
            .map_err(|_| Error::InvalidCertificate(CertificateError::BadSignature))?;

        match extract_identity_binding(cert)? {
            // Modern: a browser-compatible certificate carrying an Ed25519 identity binding.
            Some(binding) => self.verify_identity_binding(cert, &binding),
            // Legacy: an Ed25519 identity certificate pinned directly by CN and raw public key.
            None => self.verify_legacy_ed25519_pin(cert),
        }
    }

    fn verify_identity_binding(&self, cert: &X509Certificate<'_>, binding: &IdentityBinding) -> Result<(), Error> {
        if binding.public_key != self.expected_public_key {
            return Err(rejected("Identity binding public key mismatch"));
        }
        let message = binding.signed_message(cert.public_key().raw);
        let valid = signature::PublicKey::try_from(binding.public_key.as_slice())
            .and_then(|key| key.verify(&message, &binding.signature))
            .unwrap_or(false);
        if !valid {
            return Err(Error::InvalidCertificate(CertificateError::BadSignature));
        }
        Ok(())
    }

    fn verify_legacy_ed25519_pin(&self, cert: &X509Certificate<'_>) -> Result<(), Error> {
        // The CN Java reads first from an RFC 2253 name, which lists the name's parts in reverse.
        let cn = cert
            .subject()
            .iter_common_name()
            .last()
            .and_then(|cn| cn.as_str().ok())
            .ok_or_else(|| rejected("No CN in certificate"))?;
        if cn.trim() != self.expected_cn {
            return Err(rejected("CN mismatch"));
        }
        // The raw Ed25519 public key: the last 32 bytes of the SubjectPublicKeyInfo.
        let spki = cert.public_key().raw;
        if spki.len() < 32 || spki[spki.len() - 32..] != self.expected_public_key[..] {
            return Err(rejected("Public key mismatch"));
        }
        Ok(())
    }
}

// The certificate's identity binding: a current one wherever it appears; a first-form one only when
// the certificate carries no current one.
fn extract_identity_binding(cert: &X509Certificate<'_>) -> Result<Option<IdentityBinding>, Error> {
    let mut legacy = None;
    for extension in cert.extensions() {
        let ParsedExtension::IssuerAlternativeName(names) = extension.parsed_extension() else {
            continue;
        };
        for name in &names.general_names {
            let GeneralName::URI(uri) = name else {
                continue;
            };
            match parse_identity_binding(uri).map_err(|e| rejected(&e))? {
                Some(binding) if !binding.legacy => return Ok(Some(binding)),
                Some(binding) if legacy.is_none() => legacy = Some(binding),
                _ => {}
            }
        }
    }
    Ok(legacy)
}

fn rejected(reason: &str) -> Error {
    Error::InvalidCertificate(CertificateError::Other(rustls::OtherError(Arc::new(Rejected(reason.into())))))
}

#[derive(Debug)]
struct Rejected(String);

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Rejected {}

impl ServerCertVerifier for HybridTrustManager {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let (_, cert) = X509Certificate::from_der(end_entity)
            .map_err(|_| Error::InvalidCertificate(CertificateError::BadEncoding))?;
        if cert.subject().as_raw() == cert.issuer().as_raw() {
            self.check_self_signed(&cert, now)?;
            Ok(ServerCertVerified::assertion())
        } else {
            self.default
                .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// A TLS client configuration for an HTTP client talking to the super node `expected` (TLS 1.2 and
/// 1.3, HTTP/2 and HTTP/1.1), trusting its certificate as [`HybridTrustManager`] does. For reqwest:
/// `ClientBuilder::tls_backend_preconfigured(client_config(&node_id)?)`.
pub fn client_config(expected: &Id) -> Result<ClientConfig, Error> {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let verifier = HybridTrustManager::new(expected, provider.clone())?;
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}
