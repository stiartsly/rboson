
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use boson::{
    CryptoIdentity,
    did::{
        constants,
        w3c::VerifiablePresentation as VP,
		w3c::VerifiableCredential as VC,
		DIDUrl,
		Vouch
	},
};

#[test]
fn test_simple_vp() {
    let identity = CryptoIdentity::new();
    let rc = VP::builder(identity.clone())
        .with_credential_by_claims::<String>(
            "profile",
            "BosonProfile",
            vec!["https://example.com/credentials/profile/v1"],
            HashMap::from([
                ("name", "Bob".to_string()),
                ("email", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==".to_string()),
            ])
        ).unwrap().build();

    assert!(rc.is_ok());
    let vp = rc.unwrap();
    // println!("VP: {}", vp);

	let mut contexts = vp.contexts();
	contexts.sort();
	assert_eq!(contexts.len(), 3);

	let mut expected = vec![
		constants::W3C_VC_CONTEXT,
		constants::BOSON_VC_CONTEXT,
		constants::W3C_ED25519_CONTEXT,
	];
	expected.sort();
	assert_eq!(contexts, expected);

	assert_eq!(vp.holder(), identity.id());
	assert_eq!(vp.credentials().len(), 1);

	assert!(vp.credential("profile").is_some());
	let id = format!("did:boson:{}#profile", vp.holder().to_string());
	assert!(vp.credential(&id).is_some());

	let did_url = id.parse::<DIDUrl>().unwrap();
	assert!(vp.credential_by_didurl(&did_url).is_some());

	let creds = vp.credentials_by_type("BosonProfile");
	assert_eq!(creds.len(), 1);

	let profile_cred = vp.credential("profile").unwrap();
	assert!(profile_cred.is_genuine());
	assert!(profile_cred.is_valid());
	assert!(profile_cred.self_issued());

	assert!(vp.is_genuine());
	assert!(vp.validate().is_ok());

	let json = serde_json::to_string(&vp).unwrap();
	let rc = serde_json::from_str::<VP>(&json);
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let rc = json.parse::<VP>();
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let rc = VP::try_from(json.as_str());
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let cbor = serde_cbor::to_vec(&vp).unwrap();
	// println!("VP CBOR: {:?}", cbor);
	let rc = serde_cbor::from_slice::<VP>(&cbor);
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let rc = VP::try_from(cbor.as_slice());
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let vouch = vp.to_boson_vouch();
	// println!("Vouch: {}", vouch);
	assert_eq!(vouch.holder(), identity.id());
	assert_eq!(vouch.credentials().len(), 1);

	let vp_new = VP::from(&vouch);
	assert_eq!(vp, vp_new);
	let vp_new = VP::from_vouch(&vouch);
	assert_eq!(vp, vp_new);
	let vp_new = VP::from_vouch_with_type_contexts(&vouch, HashMap::from([
		("BosonProfile", vec!["https://example.com/credentials/profile/v1".to_string()])
	]));
	assert_eq!(vp, vp_new);
	let vp_new = vouch.vp().unwrap().clone();
	assert_eq!(vp, vp_new);
}

#[test]
fn test_complex_vp() {
	let identity = CryptoIdentity::new();
	let day: u64 = 24 * 60 * 60;

	let rc = VP::builder(identity.clone())
		.with_id("testVP").unwrap()
		.with_types("TestPresentation", vec!["https://example.com/presentations/test/v1"]).unwrap()
		.with_credential({
			let rc = VC::builder(identity.clone())
				.with_id("profile").unwrap()
				.with_type("BosonProfile", "https://example.com/credentials/profile/v1").unwrap()
				.with_type("Email", "https://example.com/credentials/email/v1").unwrap()
				.with_name("John's Profile")
				.with_description("This is a test profile")
				.with_valid_from(SystemTime::now())
				.with_valid_until(SystemTime::now() + Duration::from_secs(day * 30))
				.with_claims(HashMap::from([
					("name", "John Doe".to_string()),
					("avatar", "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg==".to_string()),
					("email", "cV9dX@example.com".to_string()),
					("phone", "+1-123-456-7890".to_string()),
					("address", "123 Main St, Anytown, USA".to_string()),
					("city", "Anytown".to_string()),
					("state", "CA".to_string()),
					("zip", "12345".to_string()),
					("country", "USA".to_string()),
				]))
				.build();
			assert!(rc.is_ok());
			rc.unwrap()
		})
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
				//("expiration", (SystemTime::now() + Duration::from_secs(day * 30)).duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs().to_string
			])
		).unwrap()
		.build();
	assert!(rc.is_ok());
	let vp = rc.unwrap();

	let mut ctxts = vp.contexts();
	ctxts.sort();
	assert_eq!(ctxts.len(), 4);

	let mut expected = vec![
		constants::W3C_VC_CONTEXT,
		constants::BOSON_VC_CONTEXT,
		constants::W3C_ED25519_CONTEXT,
		"https://example.com/presentations/test/v1",
	];
	expected.sort();
	assert_eq!(ctxts, expected);

	let did_url = DIDUrl::new(identity.id(), None, None, Some("testVP"));
	assert_eq!(Some(did_url.to_string().as_str()), vp.id());

	let mut types = vp.types();
	types.sort();
	assert_eq!(types.len(), 2);

	let mut expected = vec![
		constants::DEFAULT_VP_TYPE,
		"TestPresentation",
	];
	expected.sort();
	assert_eq!(types, expected);
	assert_eq!(vp.holder(), identity.id());

	let creds = vp.credentials();
	assert_eq!(creds.len(), 3);
	assert!(vp.credential("profile").is_some());

	let did_url = format!("did:boson:{}#profile", vp.holder());
	assert!(vp.credential(&did_url).is_some());

	let creds = vp.credentials_by_type("BosonProfile");
	for cred in &creds {
		assert!(cred.is_genuine());
		assert!(cred.is_valid());
		assert!(cred.self_issued());
	}

	assert!(vp.is_genuine());
	assert!(vp.validate().is_ok());

	let json = serde_json::to_string(&vp).unwrap();
	let rc = serde_json::from_str::<VP>(&json);
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let rc = json.parse::<VP>();
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let rc = VP::try_from(json.as_str());
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let cbor = serde_cbor::to_vec(&vp).unwrap();
	// println!("VP CBOR: {:?}", cbor);
	let rc = serde_cbor::from_slice::<VP>(&cbor);
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let rc = VP::try_from(cbor.as_slice());
	assert!(rc.is_ok());
	let vp_new = rc.unwrap();
	assert_eq!(vp, vp_new);

	let vouch = vp.to_boson_vouch();
	// println!("Vouch: {}", vouch);
	assert_eq!(vouch.holder(), identity.id());
	assert_eq!(vouch.credentials().len(), 3);
	assert!(vouch.is_genuine());
	assert!(vouch.validate().is_ok());

	let vp_new = VP::from(&vouch);
	assert_eq!(vp, vp_new);
	let vp_new = VP::from_vouch(&vouch);
	assert_eq!(vp, vp_new);
	let vp_new = VP::from_vouch_with_type_contexts(&vouch, HashMap::from([
		("BosonProfile", vec!["https://example.com/credentials/profile/v1".to_string()]),
		("Passport", vec!["https://example.com/credentials/passport/v1".to_string()]),
		("Email", vec!["https://example.com/credentials/email/v1".to_string()]),
		("DriverLicense", vec!["https://example.com/credentials/driverLicense/v1".to_string()]),
	]));
	assert_eq!(vp, vp_new);

	let vp_new = vouch.vp().unwrap().clone();
	assert_eq!(vp, vp_new);

	let bytes = Vec::<u8>::from(&vouch);
	let vouch_new = Vouch::try_from(bytes.as_slice()).unwrap();
	assert_eq!(vouch, vouch_new);
	assert_eq!(vouch.to_string(), vouch_new.to_string());

	let vp_new = VP::from_vouch_with_type_contexts(&vouch, HashMap::from([
		("TestPresentation", vec!["https://example.com/presentations/test/v1".to_string()]),
		("BosonProfile", vec!["https://example.com/credentials/profile/v1".to_string()]),
		("Passport", vec!["https://example.com/credentials/passport/v1".to_string()]),
		("Email", vec!["https://example.com/credentials/email/v1".to_string()]),
		("DriverLicense", vec!["https://example.com/credentials/driverLicense/v1".to_string()]),
	]));
	assert_eq!(vp, vp_new);
	assert_eq!(vp.to_string(), vp_new.to_string());

	let vouch_new = vp_new.to_boson_vouch();
	assert_eq!(vouch, vouch_new);
	assert_eq!(vouch.to_string(), vouch_new.to_string());
}

#[test]
#[should_panic]
fn test_empty_vp() {
	assert!(false, "VP should not be empty");
}
