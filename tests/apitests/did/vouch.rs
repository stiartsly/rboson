use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use boson::{
	signature,
	CryptoIdentity,
	did::{Vouch, Credential}
};

#[test]
fn test_simple_vouch() {
	let identity = CryptoIdentity::new();
	let claims = HashMap::from([
		("name", "John Doe".to_string()),
		("email", "cV9dX@example.com".to_string()),
	]);
	let rc = Vouch::builder(identity.clone())
		.with_credential_by_claims::<String>(
			"profile",
			"BosonProfile",
			claims
		).unwrap()
		.build();
	assert!(rc.is_ok());
	let vouch = rc.unwrap();
	println!("vouch: {}", vouch);

	assert_eq!(vouch.id(), None);
	assert_eq!(vouch.holder(), identity.id());
	assert_eq!(vouch.credentials().len(), 1);
	assert_eq!(vouch.credentials_by_type("BosonProfile").len(), 1);
	assert_eq!(vouch.credentials_by_id("profile").len(), 1);
	assert_eq!(vouch.signed_at().is_some(), true);
	assert_eq!(vouch.signature().len(), signature::Signature::BYTES);

	for cred in vouch.credentials() {
		assert!(cred.is_genuine());
		assert!(cred.is_valid());
		assert!(cred.self_issued());
	}

	assert!(vouch.is_genuine());
	assert!(vouch.validate().is_ok());

	let json = serde_json::to_string(&vouch).unwrap();
	println!("vouch json: {}", json);
	let rc = serde_json::from_str::<Vouch>(&json);
	assert!(rc.is_ok());
	let vouch2 = rc.unwrap();
	assert_eq!(vouch, vouch2);
	assert_eq!(vouch.to_string(), vouch2.to_string());

	let cbor = serde_cbor::to_vec(&vouch).unwrap();
	println!("vouch cbor: {:?}", cbor);
	let rc = serde_cbor::from_slice::<Vouch>(&cbor);
	assert!(rc.is_ok());
	let vouch3 = rc.unwrap();
	assert_eq!(vouch, vouch3);
	assert_eq!(vouch.to_string(), vouch3.to_string());
}

#[test]
fn test_complex_vouch() {
	let identity = CryptoIdentity::new();
	let day: u64 = 24 * 60 * 60;

	let rc = Credential::builder(identity.clone())
		.with_id("profile")
		.with_type("BosonProfile")
		.with_type("TestProfile")
		.with_name("John's Profile")
		.with_description("This is a test profile")
		.with_valid_from(SystemTime::now())
		.with_valid_until(SystemTime::now() + Duration::from_secs(day * 30))
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
	let profile_cred = rc.unwrap();
	let rc = Vouch::builder(identity.clone())
		.with_id("testVouch")
		.with_type("BosonVouch")
		.with_type("TestVouch")
		.with_credential(profile_cred)
		.with_credential_by_claims(
			"passport",
			"Passport",
			HashMap::from([
				("name", "John Doe".to_string()),
				("number", "123456789".to_string()),
			])
		).unwrap()
		.with_credential_by_claims(
			"driverLicense",
			"DriverLicense",
			HashMap::from([
				("name", "John Doe".to_string()),
				("number", "123456789".to_string()),
				//("expiration", (SystemTime::now() + Duration::from_millis(day * 30)).duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis().to_string()),
			])
		).unwrap()
		.build();
	assert!(rc.is_ok());
	let vouch = rc.unwrap();
	println!("vouch: {}", vouch);
	assert_eq!(vouch.id(), Some("testVouch"));
	assert_eq!(vouch.holder(), identity.id());
	assert_eq!(vouch.types().len(), 2);
	assert_eq!(vouch.types()[0], "BosonVouch");
	assert_eq!(vouch.types()[1], "TestVouch");

	assert_eq!(vouch.credentials_by_id("profile").len(), 1);
	assert_eq!(vouch.credentials_by_id("passport").len(), 1);
	assert_eq!(vouch.credentials_by_id("driverLicense").len(), 1);

	assert_eq!(vouch.credentials_by_type("BosonProfile").len(), 1);
	assert_eq!(vouch.credentials_by_type("TestProfile").len(), 1);
	assert_eq!(vouch.credentials_by_type("Passport").len(), 1);
	assert_eq!(vouch.credentials_by_type("DriverLicense").len(), 1);

	let creds = vouch.credentials();
	assert_eq!(creds.len(), 3);
	let ids = creds.iter().map(|c| c.id()).collect::<Vec<_>>();
	assert_eq!(ids.len(), 3);
	assert!(ids.contains(&"profile"));
	assert!(ids.contains(&"passport"));
	assert!(ids.contains(&"driverLicense"));

	assert!(vouch.is_genuine());
	assert!(vouch.validate().is_ok());

	let json = serde_json::to_string(&vouch).unwrap();
	println!("vouch json: {}", json);
	let rc = serde_json::from_str::<Vouch>(&json);
	assert!(rc.is_ok());
	let vouch2 = rc.unwrap();
	assert_eq!(vouch, vouch2);
	assert_eq!(vouch.to_string(), vouch2.to_string());

	let cbor = serde_cbor::to_vec(&vouch).unwrap();
	println!("vouch cbor: {:?}", cbor);
	let rc = serde_cbor::from_slice::<Vouch>(&cbor);
	assert!(rc.is_ok());
	let vouch3 = rc.unwrap();
	assert_eq!(vouch, vouch3);
	assert_eq!(vouch.to_string(), vouch3.to_string());
}

#[test]
fn test_empty_vouch() {
	let rc = Vouch::builder(CryptoIdentity::new()).build();
	assert!(rc.is_err());
}
