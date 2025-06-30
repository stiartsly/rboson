use boson::Id;
use boson::did::{
    VerificationMethod,
    VerificationMethodType
};

#[test]
fn test_verification_method_serde_entity() {
    let json = "{\
        \"Entity\":{\
            \"id\":\"did:boson:D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv#key-1\",\
            \"type\":\"Ed25519VerificationKey2020\",\
            \"controller\":\"D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv\",\
            \"publicKeyMultibase\":\"D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv\"\
        }\
    }";

    let rc = serde_json::from_str::<VerificationMethod>(&json);
    assert!(rc.is_ok());

    let vm = rc.unwrap();
    assert_eq!(vm.is_reference(), false);

    let controller = Id::try_from("D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv").unwrap();
    let id = format!("{}#key-1", controller.to_did_string());
    assert_eq!(vm.id(), &id);
    assert_eq!(vm.method_type(), Some(VerificationMethodType::Ed25519VerificationKey2020));
    assert_eq!(vm.controller(), Some(&controller));
    assert_eq!(vm.public_key_multibase(), Some(controller.to_base58().as_str()));
}

#[test]
fn test_verification_method_serde_reference() {
    let json = "{\
        \"Reference\":{\
            \"id\":\"did:boson:4K6hgZPEAcQT1hLkADbapDAoPhnfhbWUtcEYseQ1i8as#key-1\",\
            \"entity\":null\
        }\
    }";

    let rc = serde_json::from_str::<VerificationMethod>(&json);
    assert!(rc.is_ok());

    let id = "did:boson:4K6hgZPEAcQT1hLkADbapDAoPhnfhbWUtcEYseQ1i8as#key-1";
    let vmr = rc.unwrap();
    assert_eq!(vmr.is_reference(), true);
    assert_eq!(vmr.id(), id);
    assert_eq!(vmr.method_type(), None);
    assert_eq!(vmr.controller(), None);
}

#[test]
fn test_verification_method_serde_reference_with_entity() {
    let json = "{\
        \"Reference\":{\
            \"id\":\"did:boson:BkRUPtGbaz3aGEHyJnRc7oFrdBt1LiYduycRUmNr76ng#key-1\",\
            \"entity\":{\
                \"id\":\"did:boson:BkRUPtGbaz3aGEHyJnRc7oFrdBt1LiYduycRUmNr76ng#key-1\",\
                \"type\":\"Ed25519VerificationKey2020\",\
                \"controller\":\"BkRUPtGbaz3aGEHyJnRc7oFrdBt1LiYduycRUmNr76ng\",\
                \"publicKeyMultibase\":\"BkRUPtGbaz3aGEHyJnRc7oFrdBt1LiYduycRUmNr76ng\"\
            }\
        }\
    }";

    let rc = serde_json::from_str::<VerificationMethod>(&json);
    assert!(rc.is_ok());

    let controller = Id::try_from("BkRUPtGbaz3aGEHyJnRc7oFrdBt1LiYduycRUmNr76ng").unwrap();
    let id = format!("{}#key-1", controller.to_did_string());
    let vmr = rc.unwrap();
    assert_eq!(vmr.is_reference(), true);
    assert_eq!(vmr.id(), id);
    assert_eq!(vmr.method_type(), Some(VerificationMethodType::Ed25519VerificationKey2020));
    assert_eq!(vmr.controller(), Some(&controller));
    assert_eq!(vmr.public_key_multibase(), Some(controller.to_base58().as_str()));
}