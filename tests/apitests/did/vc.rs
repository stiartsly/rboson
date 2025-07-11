use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use boson::{
	Id,
    CryptoIdentity,
    did::{
		constants,
        DIDUrl,
        w3c::VerifiableCredential as VC,
    }
};

#[test]
fn test_simple_vc() {
    let identity = CryptoIdentity::new();
    let rc = VC::builder(identity.clone())
        .with_id("test").unwrap()
        .with_types("Passport", vec!["https://example.com/credentials/passport/v1"]).unwrap()
        .with_claim("name", "John Doe")
        .with_claim("passport", "123456789")
        .build();

    assert!(rc.is_ok());
    let vc = rc.unwrap();

    let canonical_id = DIDUrl::new(
        identity.id(),
        None,
        None,
        Some("test")
    );

	assert_eq!(vc.id(), canonical_id.to_string());
	assert_eq!(vc.types().len(), 2);

	let mut types = vc.types().into_iter().collect::<Vec<_>>();
	types.sort();
	let mut expected = vec![
		constants::DEFAULT_VC_TYPE,
		"Passport"
	];
	expected.sort();
	assert_eq!(types, expected);

	let mut ctxts = vc.contexts().iter().map(|v| v.to_string()).collect::<Vec<_>>();
	ctxts.sort();
	assert_eq!(ctxts.len(), 4);

	let mut expected = vec![
		constants::W3C_VC_CONTEXT,
		constants::BOSON_VC_CONTEXT,
		constants::W3C_ED25519_CONTEXT,
		"https://example.com/credentials/passport/v1"
	];
	expected.sort();
	assert_eq!(ctxts, expected);

	assert_eq!(vc.issuer(), identity.id());
	assert_eq!(vc.subject().id(), identity.id());

	let claims: HashMap<&str, String> = vc.subject().claims();
	assert_eq!(claims.len(), 2);
	assert_eq!(claims.get("name").unwrap(), "John Doe");
	assert_eq!(claims.get("passport").unwrap(), "123456789");

	assert!(vc.self_issued());
	assert!(vc.is_valid());
	assert!(vc.is_genuine());
	assert!(vc.validate().is_ok());

	let json = serde_json::to_string(&vc).unwrap();
	//println!("VC json: {}", json);
	let rc = serde_json::from_str::<VC>(&json);
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let rc = json.parse::<VC>();
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let rc = VC::try_from(json.as_str());
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let cbor = serde_cbor::to_vec(&vc).unwrap();
	//println!("VC cbor: {:?}", cbor);
	let rc = serde_cbor::from_slice::<VC>(&cbor);
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let cred = vc.to_boson_credential();
	//println!("Credential: {}", cred);
	let vc_new = VC::from(&cred);
	assert_eq!(vc, vc_new);
}

#[test]
fn test_complex_vc() {
	let identity = CryptoIdentity::new();
	let subject = Id::random();
	let day: u64 = 24 * 60 * 60;

	println!("Identity: {}", identity.id());
	println!("Subject: {}", subject);

	let rc = VC::builder(identity.clone())
		.with_id("fullProfile").unwrap()
		.with_type("Profile", "https://example.com/credentials/profile/v1").unwrap()
		.with_type("Passport", "https://example.com/credentials/passport/v1").unwrap()
		.with_type("Email", "https://example.com/credentials/email/v1").unwrap()
		.with_name("John Doe's Profile")
		.with_description("This is a test profile")
		.with_subject(subject.clone())
		.with_valid_from(SystemTime::now() - Duration::from_secs(day))
		.with_valid_until(SystemTime::now() + Duration::from_secs(day * 30))
		.with_claim("name", "John Doe")
		.with_claim("avatar", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==")
		.with_claim("email", "cV9dX@example.com")
		.with_claim("passport", "123456789")
		.with_claim("phone", "+1-123-456-7890")
		.with_claim("address", "123 Main St, Anytown, USA")
		.with_claim("city", "Anytown")
		.with_claim("state", "CA")
		.with_claim("zip", "12345")
		.with_claim("country", "USA")
		.with_claim("credit", 9600)
		.build();

	assert!(rc.is_ok());
	let vc = rc.unwrap();

	let _canonical_id = DIDUrl::new(
		identity.id(),
		None,
		None,
		Some("fullProfile")
	);

	// TODO: assert_eq!(vc.id(), canonical_id.to_string());

	let mut types = vc.types();
	types.sort();
	assert_eq!(types.len(), 4);

	let mut expected = vec![
		constants::DEFAULT_VC_TYPE,
		"Profile",
		"Passport",
		"Email"
	];
	expected.sort();
	assert_eq!(types, expected);

	let mut ctxts = vc.contexts();
	ctxts.sort();
	assert_eq!(ctxts.len(), 6);

	let mut expected = vec![
		constants::W3C_VC_CONTEXT,
		constants::BOSON_VC_CONTEXT,
		constants::W3C_ED25519_CONTEXT,
		"https://example.com/credentials/profile/v1",
		"https://example.com/credentials/passport/v1",
		"https://example.com/credentials/email/v1"
	];
	expected.sort();
	assert_eq!(ctxts, expected);

	assert_eq!(vc.name(), Some("John Doe's Profile"));
	assert_eq!(vc.description(), Some("This is a test profile"));

	assert_eq!(identity.id(), vc.issuer());
	assert_eq!(&subject, vc.subject().id());

	let subject = vc.subject();
	assert_eq!(subject.claims::<String>().len(), 10);
	assert_eq!(subject.claims::<i64>().len(), 1);

	assert_eq!(subject.claim::<String>("name"), Some("John Doe".to_string()));
	assert_eq!(subject.claim::<String>("avatar"), Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==".to_string()));
	assert_eq!(subject.claim::<String>("email"), Some("cV9dX@example.com".to_string()));
	assert_eq!(subject.claim::<String>("passport"), Some("123456789".to_string()));
	assert_eq!(subject.claim::<String>("phone"), Some("+1-123-456-7890".to_string()));
	assert_eq!(subject.claim::<String>("address"), Some("123 Main St, Anytown, USA".to_string()));
	assert_eq!(subject.claim::<String>("city"), Some("Anytown".to_string()));
	assert_eq!(subject.claim::<String>("state"), Some("CA".to_string()));
	assert_eq!(subject.claim::<String>("zip"), Some("12345".to_string()));
	assert_eq!(subject.claim::<String>("country"), Some("USA".to_string()));
	assert_eq!(subject.claim::<i64>("credit"), Some(9600));

	assert!(!vc.self_issued());
	assert!(vc.is_valid());
	assert!(vc.is_genuine());
	assert!(vc.validate().is_ok());

	let json = serde_json::to_string(&vc).unwrap();
	//println!("VC json: {}", json);
	let rc = serde_json::from_str::<VC>(&json);
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let rc = json.parse::<VC>();
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let rc = VC::try_from(json.as_str());
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let cbor = serde_cbor::to_vec(&vc).unwrap();
	//println!("VC cbor: {:?}", cbor);
	let rc = serde_cbor::from_slice::<VC>(&cbor);
	assert!(rc.is_ok());
	let vc_new = rc.unwrap();
	assert_eq!(vc, vc_new);

	let bcv = vc.to_boson_credential();
	let vc_new = VC::from(&bcv);
	assert_eq!(vc, vc_new);

	let vc_new = bcv.vc();
	assert!(vc_new.is_some());
	let vc_new = vc_new.unwrap().clone();
	assert_eq!(vc, vc_new);

	assert!(!bcv.self_issued());
	assert!(bcv.is_genuine());
	assert!(bcv.is_valid());
	assert!(bcv.validate().is_ok());

	// TODO:
}
