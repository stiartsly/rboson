use std::collections::HashMap;
use std::time::{SystemTime, Duration};

use boson::{
    Id,
    CryptoIdentity,
    did::Credential,
};

const DAY: u64= 24 * 60 * 60;

#[test]
fn test_simple_credential() {
    let issuer = CryptoIdentity::new();
    let rc = Credential::builder(issuer.clone())
        .with_id("profile")
        .with_types(vec!["ProfileCredential"])
		.with_claims({
			let mut claims: HashMap<&str, String> = HashMap::new();
			claims.insert("name", "John Doe".into());
			claims.insert("email", "example@gmail.com".into());
			claims
		}).build();

    assert!(rc.is_ok());
    let cred = rc.unwrap();
    println!("Credential: {}", cred);

    assert_eq!(cred.id(), "profile");
    assert_eq!(cred.types().len(), 1);
    assert_eq!(cred.types()[0], "ProfileCredential");
    assert_eq!(cred.issuer(), issuer.id());
    assert_eq!(cred.subject().id(), issuer.id());
    assert_eq!(cred.subject().claims::<String>().len(), 2);

    let claims = cred.subject().claims::<String>();
    assert_eq!(claims.get("name"), Some(&"John Doe".to_string()));
    assert_eq!(claims.get("email"), Some(&"example@gmail.com".to_string()));

    assert!(cred.self_issued());
    assert!(cred.is_valid());
    assert!(cred.is_genuine());
    assert!(cred.validate().is_ok());

    let json = serde_json::to_string(&cred).unwrap();
    println!("Credential JSON: {}", json);
    let rc = serde_json::from_str::<Credential>(&json);
    assert!(rc.is_ok());
    let cred2 = rc.unwrap();
    assert_eq!(cred, cred2);
    assert_eq!(cred.to_string(), cred2.to_string());

    let cbor = serde_cbor::to_vec(&cred).unwrap();
    println!("Credential CBOR: {:?}", cbor);
    let rc = serde_cbor::from_slice::<Credential>(&cbor);
    assert!(rc.is_ok());
    let cred3 = rc.unwrap();
    assert_eq!(cred, cred3);
    assert_eq!(cred.to_string(), cred3.to_string());
}

#[test]
fn test_complex_credential() {
    let issuer = CryptoIdentity::new();
    let subject = Id::random();
    let now = SystemTime::now();
    let rc = Credential::builder(issuer.clone())
        .with_id("profile")
        .with_types(vec!["Profile", "Test"])
        .with_name("Jane's Profile")
        .with_description("This is a test profile")
        .with_valid_from(now - Duration::from_secs(15 * DAY)) // 15 days ago
        .with_valid_until(now + Duration::from_secs(15 * DAY)) // 15 days in the future
        .with_subject(subject.clone())
        .with_claims({
            let mut claims: HashMap<&str, String> = HashMap::new();
            claims.insert("name", "Jane Doe".to_string());
            claims.insert("email", "cV9dX@example.com".to_string());
            claims.insert("phone", "+1-123-456-7890".to_string());
            claims.insert("address", "123 Main St, Anytown, USA".to_string());
            claims.insert("city", "Anytown".to_string());
            claims.insert("state", "CA".to_string());
            claims.insert("zip", "12345".to_string());
            claims.insert("country", "USA".to_string());
            claims
        }).build();

    assert!(rc.is_ok());
    let cred = rc.unwrap();
    println!("Credential: {}", cred);

    assert_eq!(cred.id(), "profile");
    assert_eq!(cred.types().len(), 2);
    assert_eq!(cred.types()[0], "Profile");
    assert_eq!(cred.types()[1], "Test");
    assert_eq!(cred.name(), Some("Jane's Profile"));
    assert_eq!(cred.description(), Some("This is a test profile"));
    assert_eq!(cred.issuer(), issuer.id());
    assert_eq!(cred.subject().id(), &subject);
    assert_eq!(cred.subject().claims::<String>().len(), 8);

    let claims = cred.subject().claims::<String>();
    assert_eq!(claims.get("name"), Some(&"Jane Doe".to_string()));
    assert_eq!(claims.get("email"), Some(&"cV9dX@example.com".to_string()));
    assert_eq!(claims.get("phone"), Some(&"+1-123-456-7890".to_string()));
    assert_eq!(claims.get("address"), Some(&"123 Main St, Anytown, USA".to_string()));
    assert_eq!(claims.get("city"), Some(&"Anytown".to_string()));
    assert_eq!(claims.get("state"), Some(&"CA".to_string()));
    assert_eq!(claims.get("zip"), Some(&"12345".to_string()));
    assert_eq!(claims.get("country"), Some(&"USA".to_string()));

    assert!(!cred.self_issued());
    assert!(cred.is_valid());
    assert!(cred.is_genuine());
    assert!(cred.validate().is_ok());

    let json = serde_json::to_string(&cred).unwrap();
    println!("Credential JSON: {}", json);
    let rc = serde_json::from_str::<Credential>(&json);
    assert!(rc.is_ok());
    let cred2 = rc.unwrap();
    assert_eq!(cred, cred2);
    assert_eq!(cred.to_string(), cred2.to_string());

    let cbor = serde_cbor::to_vec(&cred).unwrap();
    println!("Credential CBOR: {:?}", cbor);
    let rc = serde_cbor::from_slice::<Credential>(&cbor);
    assert!(rc.is_ok());
    let cred3 = rc.unwrap();
    assert_eq!(cred, cred3);
    assert_eq!(cred.to_string(), cred3.to_string());
}

#[test]
fn test_before_valid_period() {
    let issuer = CryptoIdentity::new();
    let subject = Id::random();
    let now = SystemTime::now();
    let rc = Credential::builder(issuer.clone())
        .with_id("profile")
        .with_type("Profile")
        .with_type("Test")
        .with_valid_from(now + Duration::from_secs(15 * DAY)) // 15 days in the future
        .with_subject(subject.clone())
        .with_claims({
            let mut claims: HashMap<&str, String> = HashMap::new();
            claims.insert("name", "John Doe".to_string());
            claims.insert("passport", "123456789".to_string());
            claims.insert("credit", "9600".to_string());
            claims.insert("avatar", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==".to_string());
            claims
        }).build();

    assert!(rc.is_ok());
    let cred = rc.unwrap();
    assert_eq!(cred.issuer(), issuer.id());
    assert_eq!(cred.subject().id(), &subject);
    assert_eq!(cred.types().len(), 2);
    assert_eq!(cred.types()[0], "Profile");
    assert_eq!(cred.types()[1], "Test");

    assert!(!cred.self_issued());
    assert!(!cred.is_valid());
    assert!(cred.is_genuine());
    assert!(cred.validate().is_err());

    let json = serde_json::to_string(&cred).unwrap();
    println!("Credential JSON: {}", json);
    let rc = serde_json::from_str::<Credential>(&json);
    assert!(rc.is_ok());
    let cred2 = rc.unwrap();
    assert_eq!(cred, cred2);
    assert_eq!(cred.to_string(), cred2.to_string());
}

#[test]
fn test_expired() {
    let issuer = CryptoIdentity::new();
    let now = SystemTime::now();

    let rc = Credential::builder(issuer.clone())
        .with_id("emailCredential")
        .with_types(vec!["Email"])
        .with_valid_from(now - Duration::from_secs(15 * DAY))   // 15 days ago
        .with_valid_until(now - Duration::from_secs(DAY))       // 1 day ago
        .with_claim::<&str>("email", "John Doe")
        .with_claim::<&str>("email", "cV9dX@example.com")
        .build();

    assert!(rc.is_ok());
    let cred = rc.unwrap();
    assert_eq!(cred.issuer(), issuer.id());
    assert_eq!(cred.subject().id(), issuer.id());

    assert!(cred.self_issued());
    assert!(!cred.is_valid());
    assert!(cred.is_genuine());
    assert!(cred.validate().is_err());

    let json = serde_json::to_string(&cred).unwrap();
    println!("Credential JSON: {}", json);
    let rc = serde_json::from_str::<Credential>(&json);
    assert!(rc.is_ok());
    let cred2 = rc.unwrap();
    assert_eq!(cred, cred2);
    assert_eq!(cred.to_string(), cred2.to_string());
}

#[ignore]
#[test]
fn test_invalid_signature() {
    let issuer = CryptoIdentity::new();
    let mut data = vec![0u8; 128]; // Example binary data
    for i in 0..data.len() {
        data[i] = i as u8;
    }

    let rc = Credential::builder(issuer.clone())
        .with_id("testCredential")
        .with_types(vec!["Binary"])
        .with_claims({
            let mut claims: HashMap<&str, String> = HashMap::new();
            claims.insert("name", "John Doe".to_string());
            //claims.insert("data", bs64::encode(&data));
            claims
        }).build();

    assert!(rc.is_ok());
    let cred = rc.unwrap();
    println!("Credential: {}", cred);

    assert_eq!(cred.issuer(), issuer.id());
    assert_eq!(cred.subject().id(), issuer.id());

    assert!(cred.self_issued());
    assert!(cred.is_valid());
    assert!(cred.is_genuine());
    assert!(cred.validate().is_ok());

    // Modify the data to invalidate the signature
    for i in 0..data.len() {
        data[i] += 1; // Change each byte
    }

    assert!(cred.self_issued());
    assert!(cred.is_valid());
    assert!(cred.is_genuine());
    assert!(cred.validate().is_err());
}
