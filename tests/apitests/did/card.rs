use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use boson::{
    Id,
    signature,
    CryptoIdentity,
    did::{Card, Credential},
};

#[test]
fn test_simple_card() {
    let identity = CryptoIdentity::new();
    let rc = Card::builder(identity.clone())
        .with_credential_by_claims(
            "profile",
            "BosonProfile",
            HashMap::from([
                ("name", "John Doe"),
                ("avatar", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==")
            ])
        ).unwrap()
        .with_service::<String>(
            "homeNode",
            "BosonHomeNode",
            &Id::random().to_string(),
            HashMap::new(),
        ).unwrap()
        .build();
    assert!(rc.is_ok());

    let card = rc.unwrap();
    assert_eq!(card.id(), identity.id());
    assert_eq!(card.credentials().len(), 1);
    assert_eq!(card.services().len(), 1);

    assert_eq!(card.credentials_by_id("profile").len(), 1);
    assert_eq!(card.services_by_id("homeNode").len(), 1);

    assert_eq!(card.credentials_by_type("BosonProfile").len(), 1);
    assert_eq!(card.services_by_type("BosonHomeNode").len(), 1);

    assert!(card.profile_credential().is_some());
    assert!(card.homenode_service().is_some());

    let profile_cred = card.profile_credential().unwrap();
    assert!(profile_cred.is_genuine());
    assert!(profile_cred.is_valid());
    assert!(profile_cred.self_issued());

    assert!(card.is_genuine());
    assert!(card.validate().is_ok());
    assert!(card.signature().len() == signature::Signature::BYTES);

    let json = serde_json::to_string(&card).unwrap();
    println!("Card JSON: {}", json);
    let rc = serde_json::from_str::<Card>(&json);
    assert!(rc.is_ok());
    let card2 = rc.unwrap();
    assert_eq!(card, card2);
    assert_eq!(card.to_string(), card2.to_string());
}

#[test]
fn test_complex_card() {
    let identity = CryptoIdentity::new();
    let day: u64 = 24 * 60 * 60 * 1000;
    let rc = Card::builder(identity.clone())
        .with_credential({
            let rc = Credential::builder(identity.clone())
                .with_id("profile")
                .with_type("BosonProfile")
                .with_type("TestProfile")
                .with_name("John's Profile")
                .with_description("This is a test profile")
                .with_valid_from(SystemTime::now())
                .with_valid_until(SystemTime::now() + Duration::from_millis(day * 30))
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
        }).unwrap()
        .with_credential_by_claims(
            "passport",
            "Passport",
            HashMap::from([
                ("name", "John Doe"),
                ("number", "123456789")
            ])
        ).unwrap()
        .with_credential_by_claims(
            "driverLicense",
            "DriverLicense",
            HashMap::from([
                ("name", "John Doe"),
                ("number", "123456789"),
                //("expiration", SystemTime::now() + Duration::from_millis(day * 30).as_millis().try_into().unwrap())
            ])
        ).unwrap()
        .with_service::<String>(
            "homeNode",
            "BosonHomeNode",
            &Id::random().to_string(),
            HashMap::from([
                ("sig", "F5r4bSbLamnvpDEiFgfuspszfMMMmQAhBdlhS1ZiliRdc4i-3aXZZ7mzYdkkpffpm3EsfwyDAcV_mwPiKf8cDA".into())
            ]),
        ).unwrap()
        .with_service::<String>(
            "messaging",
            "BMS",
            &Id::random().to_string(),
            HashMap::new(),
        ).unwrap()
        .with_service::<String>(
            "bcr",
            "CredentialRepo",
            "https://example.com/bcr",
            HashMap::from([
                ("src", "BosonCards".into()),
                ("token", "123456789".into())
            ]),
        ).unwrap()
        .build();

    assert!(rc.is_ok());
    let card = rc.unwrap();
    println!("Card: {}", card);

    assert_eq!(card.id(), identity.id());
    assert_eq!(card.credentials_by_id("profile").len(), 1);
    assert_eq!(card.credentials_by_id("passport").len(), 1);
    assert_eq!(card.credentials_by_id("driverLicense").len(), 1);
    assert_eq!(card.credentials_by_type("BosonProfile").len(), 1);
    assert_eq!(card.credentials_by_type("TestProfile").len(), 1);
    assert_eq!(card.credentials_by_type("Passport").len(), 1);
    assert_eq!(card.credentials_by_type("DriverLicense").len(), 1);
    assert_eq!(card.services_by_id("homeNode").len(), 1);
    assert_eq!(card.services_by_id("messaging").len(), 1);
    assert_eq!(card.services_by_id("bcr").len(), 1);

    assert_eq!(card.credentials().len(), 3);
    let mut ids = card.credentials()
        .iter()
        .map(|c| c.id())
        .collect::<Vec<_>>();

    ids.sort();
    assert_eq!(ids[0], "driverLicense");
    assert_eq!(ids[1], "passport");
    assert_eq!(ids[2], "profile");

    assert_eq!(card.services().len(), 3);
    let mut ids = card.services()
        .iter()
        .map(|s| s.id())
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids[0], "bcr");
    assert_eq!(ids[1], "homeNode");
    assert_eq!(ids[2], "messaging");

    assert!(card.profile_credential().is_some());
    assert!(card.homenode_service().is_some());

    let profile_cred = card.profile_credential().unwrap();
    assert!(profile_cred.is_genuine());
    assert!(profile_cred.is_valid());
    assert!(profile_cred.self_issued());

    assert!(card.is_genuine());
    assert!(card.validate().is_ok());
    assert!(card.signature().len() == signature::Signature::BYTES);

    let json = serde_json::to_string(&card).unwrap();
    println!("Card JSON: {}", json);
    let rc = serde_json::from_str::<Card>(&json);
    assert!(rc.is_ok());
    let card2 = rc.unwrap();
    assert_eq!(card, card2);
    assert_eq!(card.to_string(), card2.to_string());

    let cbor = serde_cbor::to_vec(&card).unwrap();
    println!("Card CBOR: {:?}", cbor);
    let rc = serde_cbor::from_slice::<Card>(&cbor);
    assert!(rc.is_ok());
    let card3 = rc.unwrap();
    assert_eq!(card, card3);
    assert_eq!(card.to_string(), card3.to_string());
}

#[test]
fn test_card_builder() {
    let identity = CryptoIdentity::new();
    let card = Card::builder(identity.clone()).build().unwrap();
    assert_eq!(card.id(), identity.id());
    assert!(card.credentials().is_empty());
    assert!(card.services().is_empty());
    assert!(card.is_genuine());
    assert!(card.validate().is_ok());
    assert!(card.profile_credential().is_none());
    assert!(card.homenode_service().is_none());
    assert!(card.signature().len() == signature::Signature::BYTES);

    let json = serde_json::to_string(&card).unwrap();
    println!("Card JSON: {}", json);
    let rc = serde_json::from_str::<Card>(&json);
    println!("rc: {:?}", rc);
    assert!(rc.is_ok());
    let card2 = rc.unwrap();
    assert_eq!(card, card2);
    assert_eq!(card.to_string(), card2.to_string());

    let cbor = serde_cbor::to_vec(&card).unwrap();
    println!("Card CBOR: {:?}", cbor);
    let rc = serde_cbor::from_slice::<Card>(&cbor);
    assert!(rc.is_ok());
    let card3 = rc.unwrap();
    assert_eq!(card, card3);
    assert_eq!(card.to_string(), card3.to_string());
}
