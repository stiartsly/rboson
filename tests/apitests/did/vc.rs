use boson::{
    CryptoIdentity,
    did::{
		constants,
        DIDUrl,
        w3c::VerifiableCredential as VC,
    }
};

#[ignore]
#[test]
fn test_simple_credential() {
    let identity = CryptoIdentity::new();
    let rc = VC::builder(identity.clone())
        .with_id("test").unwrap()
        .with_types("Passport", vec!["https://example.com/credentials/passport/v1".into()]).unwrap()
        .with_claim("name", "John Doe")
        .with_claim("passport", "123456789")
        .build();

    assert!(rc.is_ok());
    let vc = rc.unwrap();

    let canonical_id = DIDUrl::new(
        identity.id(),
        None,
        None,
        Some("test".into())
    );
	assert_eq!(vc.id(), canonical_id.to_string());
	assert_eq!(vc.types().len(), 2);

	let mut types = vc.types().iter().map(|v| v.to_string()).collect::<Vec<_>>();
	types.sort();
	assert_eq!(types[0], constants::DEFAULT_VC_TYPE);
	assert_eq!(types[1], "Passport");

	let mut ctxts = vc.contexts().iter().map(|v| v.to_string()).collect::<Vec<_>>();
	ctxts.sort();
	assert_eq!(ctxts.len(), 4);
	assert_eq!(ctxts[0], constants::W3C_VC_CONTEXT);
	assert_eq!(ctxts[1], constants::BOSON_VC_CONTEXT);
	assert_eq!(ctxts[2], constants::W3C_ED25519_CONTEXT);
	assert_eq!(ctxts[3], "https://example.com/credentials/passport/v1");

	assert_eq!(vc.issuer(), identity.id());
	assert_eq!(vc.subject().id(), identity.id());

	let claims = vc.subject().claims();
	assert_eq!(claims.len(), 2);
	assert_eq!(claims.get("name").unwrap(), "John Doe");
	assert_eq!(claims.get("passport").unwrap(), "123456789");

	assert!(vc.self_issued());
	assert!(vc.is_valid());
	assert!(vc.is_genuine());
	assert!(vc.validate().is_ok());

	let json = serde_json::to_string(&vc).unwrap();
	println!("VC json: {}", json);
	let rc = serde_json::from_str::<VC>(&json);
	assert!(rc.is_ok());
	let vc2 = rc.unwrap();
	assert_eq!(vc, vc2);

	let cred = vc.to_credential();
	println!("Credential: {}", cred);
	let vc2 = VC::from(&cred);
	assert_eq!(vc, vc2);
}
