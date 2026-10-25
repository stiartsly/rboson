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

//! Tests of the Boson TLS trust rules, with certificates laid out as the Java SDK's CertUtil makes them.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use boson_api::{Id, signature::KeyPair as IdentityKey};
use boson_http::tls::{self, HybridTrustManager, IdentityBinding};
use rcgen::{CertificateParams, CustomExtension, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256, PublicKeyData};
use rustls::{
    CertificateError, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::aws_lc_rs,
    pki_types::{CertificateDer, ServerName, UnixTime},
};

// A default verifier that rejects every chain, and remembers that it was asked.
#[derive(Debug, Default)]
struct Rejecting(AtomicBool);

impl ServerCertVerifier for Rejecting {
    fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, Error> {
        self.0.store(true, Ordering::SeqCst);
        Err(Error::InvalidCertificate(CertificateError::UnknownIssuer))
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        unreachable!()
    }
    fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        unreachable!()
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![]
    }
}

fn verifier(expected: &Id) -> (HybridTrustManager, Arc<Rejecting>) {
    let default = Arc::new(Rejecting::default());
    let manager = HybridTrustManager::with_default_verifier(expected, default.clone(), Arc::new(aws_lc_rs::default_provider()));
    (manager, default)
}

fn verify(manager: &HybridTrustManager, cert: &CertificateDer<'_>) -> Result<ServerCertVerified, Error> {
    let name = ServerName::try_from("node.example").unwrap();
    manager.verify_server_cert(cert, &[], &name, &[], UnixTime::now())
}

fn id_of(key: &IdentityKey) -> Id {
    Id::from(key.public_key())
}

// DER of an issuerAltName extension value: GeneralNames with uniformResourceIdentifier entries.
fn issuer_alt_names(uris: &[String]) -> Vec<u8> {
    fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
        let mut out = vec![tag];
        let len = content.len();
        if len < 0x80 {
            out.push(len as u8);
        } else if len < 0x100 {
            out.extend([0x81, len as u8]);
        } else {
            out.extend([0x82, (len >> 8) as u8, len as u8]);
        }
        out.extend_from_slice(content);
        out
    }
    let names: Vec<u8> = uris.iter().flat_map(|uri| tlv(0x86, uri.as_bytes())).collect();
    tlv(0x30, &names)
}

fn current_binding(identity: &IdentityKey, spki: &[u8]) -> String {
    let signature = identity.private_key().sign_into(&tls::cert_binding_message(spki)).unwrap();
    format!("boson:certbind:1:{}:{}", identity.public_key().to_base58(), URL_SAFE_NO_PAD.encode(signature))
}

fn legacy_binding(identity: &IdentityKey, spki: &[u8]) -> String {
    let signature = identity.private_key().sign_into(spki).unwrap();
    format!("boson:ed25519:{}:{}", identity.public_key().to_base58(), URL_SAFE_NO_PAD.encode(signature))
}

// A self-signed ECDSA P-256 certificate carrying the bindings that make_uris returns for its SPKI.
fn ecdsa_cert(make_uris: impl FnOnce(&[u8]) -> Vec<String>) -> CertificateDer<'static> {
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let uris = make_uris(&key.subject_public_key_info());
    let mut params = CertificateParams::new(vec!["node.example".to_string()]).unwrap();
    params.not_before = rcgen::date_time_ymd(2020, 1, 1);
    params.not_after = rcgen::date_time_ymd(2100, 1, 1);
    if !uris.is_empty() {
        params.custom_extensions.push(CustomExtension::from_oid_content(&[2, 5, 29, 18], issuer_alt_names(&uris)));
    }
    params.self_signed(&key).unwrap().der().clone()
}

// A legacy self-signed Ed25519 certificate made from an identity key, with the given CN.
fn ed25519_cert(identity: &IdentityKey, cn: &str) -> CertificateDer<'static> {
    let mut pkcs8 = hex_literal("302e020100300506032b657004220420");
    pkcs8.extend_from_slice(&identity.private_key().seed());
    let key = KeyPair::try_from(pkcs8.as_slice()).unwrap();
    let mut params = CertificateParams::new(vec!["node.example".to_string()]).unwrap();
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, cn);
    params.distinguished_name = name;
    params.self_signed(&key).unwrap().der().clone()
}

fn hex_literal(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

fn is_rejection(result: Result<ServerCertVerified, Error>) -> bool {
    matches!(result, Err(Error::InvalidCertificate(_)))
}

#[test]
fn current_binding_by_the_expected_identity_is_trusted() {
    let identity = IdentityKey::random();
    let cert = ecdsa_cert(|spki| vec![current_binding(&identity, spki), legacy_binding(&identity, spki)]);
    let (manager, default) = verifier(&id_of(&identity));
    assert!(verify(&manager, &cert).is_ok());
    assert!(!default.0.load(Ordering::SeqCst), "a self-signed certificate is not delegated");
}

#[test]
fn binding_by_another_identity_is_rejected() {
    let identity = IdentityKey::random();
    let cert = ecdsa_cert(|spki| vec![current_binding(&identity, spki)]);
    let (manager, _) = verifier(&id_of(&IdentityKey::random()));
    assert!(is_rejection(verify(&manager, &cert)));
}

#[test]
fn binding_over_another_key_is_rejected() {
    let identity = IdentityKey::random();
    // Signed for a different SubjectPublicKeyInfo: a binding copied onto another certificate.
    let other = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap().subject_public_key_info();
    let cert = ecdsa_cert(|_| vec![current_binding(&identity, &other)]);
    let (manager, _) = verifier(&id_of(&identity));
    assert!(matches!(verify(&manager, &cert), Err(Error::InvalidCertificate(CertificateError::BadSignature))));
}

#[test]
fn first_form_counts_only_without_a_current_one() {
    let identity = IdentityKey::random();
    let (manager, _) = verifier(&id_of(&identity));

    let legacy_only = ecdsa_cert(|spki| vec![legacy_binding(&identity, spki)]);
    assert!(verify(&manager, &legacy_only).is_ok());

    // A current binding that fails is not rescued by a valid first-form one.
    let other = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap().subject_public_key_info();
    let mixed = ecdsa_cert(|spki| vec![legacy_binding(&identity, spki), current_binding(&identity, &other)]);
    assert!(is_rejection(verify(&manager, &mixed)));
}

#[test]
fn malformed_binding_is_rejected() {
    let identity = IdentityKey::random();
    let (manager, _) = verifier(&id_of(&identity));
    for uri in ["boson:certbind:1:onlyonefield", "boson:certbind:2:a:b", "boson:ed25519:a:b:c"] {
        let cert = ecdsa_cert(|_| vec![uri.to_string()]);
        assert!(is_rejection(verify(&manager, &cert)), "{uri}");
    }
}

#[test]
fn other_uris_are_ignored() {
    let identity = IdentityKey::random();
    let cert = ecdsa_cert(|spki| vec!["https://example.com/".to_string(), current_binding(&identity, spki)]);
    let (manager, _) = verifier(&id_of(&identity));
    assert!(verify(&manager, &cert).is_ok());
}

#[test]
fn legacy_ed25519_certificate_is_pinned_by_cn_and_key() {
    let identity = IdentityKey::random();
    let id = id_of(&identity);
    let (manager, _) = verifier(&id);
    assert!(verify(&manager, &ed25519_cert(&identity, &id.to_base58())).is_ok());
    assert!(is_rejection(verify(&manager, &ed25519_cert(&identity, "someone-else"))));

    // The expected CN on a different key.
    let other = IdentityKey::random();
    assert!(is_rejection(verify(&manager, &ed25519_cert(&other, &id.to_base58()))));
}

#[test]
fn self_signed_without_binding_or_matching_pin_is_rejected() {
    let identity = IdentityKey::random();
    let (manager, _) = verifier(&id_of(&identity));
    assert!(is_rejection(verify(&manager, &ecdsa_cert(|_| vec![]))));
}

#[test]
fn validity_is_checked() {
    let identity = IdentityKey::random();
    let cert = ecdsa_cert(|spki| vec![current_binding(&identity, spki)]);
    let (manager, _) = verifier(&id_of(&identity));
    let name = ServerName::try_from("node.example").unwrap();
    let far_future = UnixTime::since_unix_epoch(std::time::Duration::from_secs(32_503_680_000)); // year 3000, past 2100
    let long_ago = UnixTime::since_unix_epoch(std::time::Duration::from_secs(0)); // 1970, before 2020
    assert!(matches!(manager.verify_server_cert(&cert, &[], &name, &[], far_future), Err(Error::InvalidCertificate(CertificateError::Expired))));
    assert!(matches!(manager.verify_server_cert(&cert, &[], &name, &[], long_ago), Err(Error::InvalidCertificate(CertificateError::NotValidYet))));
}

#[test]
fn ca_issued_chains_go_to_the_default_verifier() {
    let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.distinguished_name.push(DnType::CommonName, "Test CA");
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::new(ca_params, ca_key);

    let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let leaf = CertificateParams::new(vec!["node.example".to_string()]).unwrap().signed_by(&leaf_key, &issuer).unwrap();
    let _ = ca;

    let (manager, default) = verifier(&id_of(&IdentityKey::random()));
    assert!(matches!(verify(&manager, leaf.der()), Err(Error::InvalidCertificate(CertificateError::UnknownIssuer))));
    assert!(default.0.load(Ordering::SeqCst), "a CA-issued chain is the default verifier's to judge");
}

#[test]
fn binding_uris_parse_as_in_the_java_sdk() {
    assert_eq!(tls::parse_identity_binding("https://example.com/").unwrap(), None);
    let parsed = tls::parse_identity_binding("boson:certbind:1:2g:AQID").unwrap().unwrap();
    assert_eq!(parsed, IdentityBinding { public_key: vec![97], signature: vec![1, 2, 3], legacy: false }); // Base58 "2g" = 1 * 58 + 39
    assert_eq!(parsed.signed_message(b"spki"), b"boson:certbind:1spki");
    assert!(tls::parse_identity_binding("boson:ed25519:2g:AQID").unwrap().unwrap().legacy);
    assert!(tls::parse_identity_binding("boson:certbind:1:2g").is_err());
    assert!(tls::parse_identity_binding("boson:certbind:1:2g?:AQID").is_err());
    assert!(tls::parse_identity_binding("boson:certbind:01:2g:AQID").is_err());
}
