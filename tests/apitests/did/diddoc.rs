use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use boson::{
    Id,
    CryptoIdentity,
    did::{
        constants,
        w3c::DIDDocument as DIDDoc,
        w3c::VerifiableCredential as VC,
        VerificationMethodType,
        Card,
        DIDUrl,
    },
};

#[test]
fn test_simple_doc() {
    let identity = CryptoIdentity::new();
    let serverid = Id::random();
    let rc = DIDDoc::builder(identity.clone())
        .with_credential_by_claims::<String>(
            "profile",
            "BosonProfile",
            vec!["https://example.com/credentials/profile/v1"],
            HashMap::from([
                ("name", "Bob".to_string()),
                ("avatar", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==".to_string()),
            ])
        ).unwrap()
        .with_service(
            "homeNode",
            "BosonHomeNode",
            serverid.to_string().as_str(),
            HashMap::from([
                ("sig", "F5r4bSbLamnvpDEiFgfuspszfMMMmQAhBdlhS1ZiliRdc4i-3aXZZ7mzYdkkpffpm3EsfwyDAcV_mwPiKf8cDA".to_string())
            ])
        ).unwrap()
        .build();
    assert!(rc.is_ok());
    let doc = rc.unwrap();
    assert_eq!(doc.id(), identity.id());

    let mut ctxts = doc.contexts();
    ctxts.sort();
    assert_eq!(ctxts.len(), 3);
    let mut expected = vec![
        constants::W3C_DID_CONTEXT,
        constants::BOSON_DID_CONTEXT,
        constants::W3C_ED25519_CONTEXT,
    ];
    expected.sort();
    assert_eq!(ctxts, expected);

    let methods = doc.verification_methods();
    assert_eq!(methods.len(), 1);
    assert!(doc.verification_method("#default").is_some());
    let did_url = format!("did:boson:{}#default", doc.id());
    assert!(doc.verification_method(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.verification_method_by_didurl(&rc.unwrap()).is_some());
    let methods = doc.verification_methods_by_type(VerificationMethodType::Ed25519VerificationKey2020);
    assert_eq!(methods.len(), 1);

    let auths = doc.authentications();
    assert_eq!(auths.len(), 1);
    assert!(doc.authentication("#default").is_some());
    let did_url = format!("did:boson:{}#default", doc.id());
    assert!(doc.authentication(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.authentication_by_didurl(&rc.unwrap()).is_some());

    let asserts = doc.assertions();
    assert_eq!(asserts.len(), 1);
    assert!(doc.assertion("#default").is_some());
    let did_url = format!("did:boson:{}#default", doc.id());
    assert!(doc.assertion(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.assertion_by_didurl(&rc.unwrap()).is_some());

    let creds = doc.credentials();
    assert_eq!(creds.len(), 1);
    assert!(doc.credential("profile").is_some());
    let did_url = format!("did:boson:{}#profile", doc.id());
    assert!(doc.credential(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.credential_by_didurl(&rc.unwrap()).is_some());
    let creds = doc.credentials_by_type("BosonProfile");
    assert_eq!(creds.len(), 1);

    let rc = doc.credential("profile");
    assert!(rc.is_some());
    let cred_profile = rc.unwrap();
    assert!(cred_profile.is_genuine());
    assert!(cred_profile.is_valid());
    assert!(cred_profile.self_issued());

    let services = doc.services();
    assert_eq!(services.len(), 1);
    assert!(doc.service("homeNode").is_some());
    let did_url = format!("did:boson:{}#homeNode", doc.id());
    assert!(doc.service(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.service_by_didurl(&rc.unwrap()).is_some());
    let services = doc.services_by_type("BosonHomeNode");
    assert_eq!(services.len(), 1);

    assert!(doc.is_genuine());
    assert!(doc.validate().is_ok());

    let json = serde_json::to_string(&doc).unwrap();
    let doc_new: DIDDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let cbor = serde_cbor::to_vec(&doc).unwrap();
    let doc_new = serde_cbor::from_slice::<DIDDoc>(&cbor).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = json.parse::<DIDDoc>().unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = DIDDoc::try_from(json.as_str()).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let card = doc.to_boson_card();
    assert!(card.is_genuine());
    assert!(card.validate().is_ok());

    assert_eq!(card.credentials().len(), 1);
    assert_eq!(card.services().len(), 1);

    let doc_new = card.did_doc();
    assert!(doc_new.is_some());

    let doc_new = DIDDoc::from_card(&card);
    assert_eq!(doc, doc_new);

    let doc_new = DIDDoc::from_card_with_contexts(
        &card,
        vec![],
        HashMap::from([("BosonProfile", vec!["https://example.com/credentials/profile/v1"])]),
    );
    assert_eq!(doc, doc_new);

    let bytes = Vec::<u8>::from(&card);
    let rc = Card::try_from(bytes.as_slice());
    assert!(rc.is_ok());
    let card_new = rc.unwrap();
    assert_eq!(card, card_new);
    assert_eq!(card.to_string(), card_new.to_string());

    let doc_new = DIDDoc::from_card_with_contexts(
        &card_new,
        vec![],
        HashMap::from([("BosonProfile", vec!["https://example.com/credentials/profile/v1"])]),
    );
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let card_new = doc_new.to_boson_card();
    assert_eq!(card, card_new);
    assert_eq!(card.to_string(), card_new.to_string());
}

#[test]
fn test_complex_doc() {
    let identity = CryptoIdentity::new();
    let serverid = Id::random();
    let day: u64 = 24 * 60 * 60;

    let rc = DIDDoc::builder(identity.clone())
        .with_credential({
            let rc = VC::builder(identity.clone())
                .with_id("profile").unwrap()
                .with_type("BosonProfile", "https://example.com/credentials/profile/v1").unwrap()
                .with_type("Email", "https://example.com/credentials/email/v1").unwrap()
                .with_name("John's Profile")
                .with_description("This is a test profile")
                .with_valid_from(SystemTime::now())
                .with_valid_until(SystemTime::now() + Duration::from_secs(30 *day)) // 30 days
                .with_claim("name", "John Doe")
                .with_claim("avatar", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==")
                .with_claim("email", "cV9dX@example.com")
                .with_claim("phone", "+1-123-456-7890")
                .with_claim("address", "123 Main St, Anytown, USA")
                .with_claim("city", "Anytown")
                .with_claim("state", "CA")
                .with_claim("zip", "12345")
                .with_claim("country", "USA")
                .build();
            assert!(rc.is_ok());
            rc.unwrap()
        }).map_err(|e| {
            eprintln!("Error creating credential: {}", e);
            assert!(false);
            e
        }).unwrap()
        .with_credential_by_claims::<String>(
            "passport",
            "Passport",
            vec!["https://example.com/credentials/passport/v1"],
            HashMap::from([
                ("name", "John Doe".to_string()),
                ("number", "123456789".to_string()),
            ])
        ).unwrap()
        .with_credential_by_claims::<String>(
            "driverLicense",
            "DriverLicense",
            vec!["https://example.com/credentials/driverLicense/v1"],
            HashMap::from([
                ("name", "John Doe".to_string()),
                ("number", "123456789".to_string()),
               // ("expiration", (DIDDoc::now() + 30 * day).to_string
            ])
        ).map_err(|e| {
            eprintln!("Error creating credential: {}", e);
            assert!(false);
            e
        }).unwrap()
        .with_service(
            "homeNode",
            "BosonHomeNode",
            serverid.to_string().as_str(),
            HashMap::from([
                ("sig", "F5r4bSbLamnvpDEiFgfuspszfMMMmQAhBdlhS1ZiliRdc4i-3aXZZ7mzYdkkpffpm3EsfwyDAcV_mwPiKf8cDA".to_string())
            ])
        ).map_err(|e| {
            eprintln!("Error creating service: {}", e);
            assert!(false);
            e
        }).unwrap()
        .with_service::<String>(
            "messaging",
            "BMS",
            Id::random().to_string().as_str(),
            HashMap::new()
        ).map_err(|e| {
            eprintln!("Error creating service: {}", e);
            assert!(false);
            e
        }).unwrap()
        .with_service(
            "bcr",
            "CredentialRepo",
            "https://example.com/bcr",
            HashMap::from([
                ("src", "BosonCards".to_string()),
                ("token", "123456789".to_string())
            ])
        ).map_err(|e| {
            eprintln!("Error creating service: {}", e);
            assert!(false);
            e
        }).unwrap()
        .build();
    assert!(rc.is_ok());
    let doc = rc.unwrap();
    assert_eq!(doc.id(), identity.id());

    let mut ctxts = doc.contexts();
    ctxts.sort();
    assert_eq!(ctxts.len(), 3);
    let mut expected = vec![
        constants::W3C_DID_CONTEXT,
        constants::BOSON_DID_CONTEXT,
        constants::W3C_ED25519_CONTEXT,
    ];
    expected.sort();
    assert_eq!(ctxts, expected);

    let methods = doc.verification_methods();
    assert_eq!(methods.len(), 1);
    assert!(doc.verification_method("#default").is_some());
    let did_url = format!("did:boson:{}#default", doc.id());
    assert!(doc.verification_method(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.verification_method_by_didurl(&rc.unwrap()).is_some());
    let methods = doc.verification_methods_by_type(VerificationMethodType::Ed25519VerificationKey2020);
    assert_eq!(methods.len(), 1);

    let auths = doc.authentications();
    assert_eq!(auths.len(), 1);
    assert!(doc.authentication("#default").is_some());
    let did_url = format!("did:boson:{}#default", doc.id());
    assert!(doc.authentication(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.authentication_by_didurl(&rc.unwrap()).is_some());

    let asserts = doc.assertions();
    assert_eq!(asserts.len(), 1);
    assert!(doc.assertion("#default").is_some());
    let did_url = format!("did:boson:{}#default", doc.id());
    assert!(doc.assertion(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.assertion_by_didurl(&rc.unwrap()).is_some());

    let creds = doc.credentials();
    assert_eq!(creds.len(), 3);
    assert!(doc.credential("profile").is_some());
    let did_url = format!("did:boson:{}#profile", doc.id());
    assert!(doc.credential(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.credential_by_didurl(&rc.unwrap()).is_some());

    for cred in creds {
        assert!(cred.is_genuine());
        assert!(cred.is_valid());
        assert!(cred.self_issued());
    }

    let servces = doc.services();
    assert_eq!(servces.len(), 3);
    assert!(doc.service("homeNode").is_some());
    let did_url = format!("did:boson:{}#homeNode", doc.id());
    assert!(doc.service(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.service_by_didurl(&rc.unwrap()).is_some());

    let services = doc.services_by_type("BosonHomeNode");
    assert_eq!(services.len(), 1);

    assert!(doc.service("messaging").is_some());
    let did_url = format!("did:boson:{}#messaging", doc.id());
    assert!(doc.service(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.service_by_didurl(&rc.unwrap()).is_some());

    let services = doc.services_by_type("CredentialRepo");
    assert_eq!(services.len(), 1);

    assert!(doc.service("bcr").is_some());
    let did_url = format!("did:boson:{}#bcr", doc.id());
    assert!(doc.service(&did_url).is_some());
    let rc = did_url.parse::<DIDUrl>();
    assert!(rc.is_ok());
    assert!(doc.service_by_didurl(&rc.unwrap()).is_some());

    assert!(doc.is_genuine());
    assert!(doc.validate().is_ok());

    let json = serde_json::to_string(&doc).unwrap();
    let doc_new: DIDDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let cbor = serde_cbor::to_vec(&doc).unwrap();
    let doc_new = serde_cbor::from_slice::<DIDDoc>(&cbor).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = json.parse::<DIDDoc>().unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = DIDDoc::try_from(json.as_str()).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let card = doc.to_boson_card();
    assert!(card.is_genuine());
    assert!(card.validate().is_ok());
    assert_eq!(card.credentials().len(), 3);
    assert_eq!(card.services().len(), 3);

    let doc_new = card.did_doc();
    assert!(doc_new.is_some());
    assert_eq!(doc, doc_new.unwrap().clone());

    let doc_new = DIDDoc::from_card(&card);
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = DIDDoc::from_card_with_contexts(
        &card,
        vec![],
        HashMap::from([
            ("BosonProfile", vec!["https://example.com/credentials/profile/v1"]),
            ("Email", vec!["https://example.com/credentials/email/v1"]),
            ("Passport", vec!["https://example.com/credentials/passport/v1"]),
            ("DriverLicense", vec!["https://example.com/credentials/driverLicense/v1"]),
        ]),
    );
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());
    assert_eq!(doc_new.to_boson_card(), card);

    let bytes_vec = Vec::<u8>::from(&card);
    let bytes = bytes_vec.as_slice();
    let rc = Card::try_from(bytes);
    assert!(rc.is_ok());
    let card_new = rc.unwrap();
    assert_eq!(card, card_new);
    assert_eq!(card.to_string(), card_new.to_string());
}

#[test]
fn test_empty_doc() {
    let identity = CryptoIdentity::new();
    let rc = DIDDoc::builder(identity.clone()).build();
    assert!(rc.is_ok());
    let doc = rc.unwrap();

    assert_eq!(doc.id(), identity.id());
    assert_eq!(doc.verification_methods().len(), 1);
    assert_eq!(doc.authentications().len(), 1);
    assert_eq!(doc.assertions().len(), 1);
    assert_eq!(doc.credentials().len(), 0);
    assert_eq!(doc.services().len(), 0);
    assert!(doc.is_genuine());
    assert!(doc.validate().is_ok());

    let json = serde_json::to_string(&doc).unwrap();
    let doc_new: DIDDoc = serde_json::from_str(&json).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let cbor = serde_cbor::to_vec(&doc).unwrap();
    let doc_new = serde_cbor::from_slice::<DIDDoc>(&cbor).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = json.parse::<DIDDoc>().unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = DIDDoc::try_from(json.as_str()).unwrap();
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let card = doc.to_boson_card();
    assert!(card.is_genuine());
    assert!(card.validate().is_ok());
    assert_eq!(card.credentials().len(), 0);
    assert_eq!(card.services().len(), 0);

    let doc_new = card.did_doc();
    assert!(doc_new.is_some());
    assert_eq!(doc, doc_new.unwrap().clone());

    let doc_new = DIDDoc::from_card(&card);
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());

    let doc_new = DIDDoc::from_card_with_contexts(
        &card,
        vec![],
        HashMap::new(),
    );
    assert_eq!(doc, doc_new);
    assert_eq!(doc.to_string(), doc_new.to_string());
}
