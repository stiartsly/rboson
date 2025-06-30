use crate::Id;
use crate::did::{
    VerificationMethod,
    VerificationMethodType
};

#[test]
fn test_verification_method_serde_entity() {
    let controller = Id::random();
    let id = format!("{}#key-1", controller.to_did_string());
    let vm = VerificationMethod::entity(
        id.clone(),
        VerificationMethodType::Ed25519VerificationKey2020,
        controller.clone(),
        controller.to_base58()
    );

    assert_eq!(vm.is_reference(), false);
    assert_eq!(vm.id(), &id);
    assert_eq!(vm.method_type(), Some(VerificationMethodType::Ed25519VerificationKey2020));
    assert_eq!(vm.controller(), Some(&controller));

    let json = serde_json::to_string(&vm).unwrap();
    println!("entity json - {}", json);
    println!("vm {}", vm);

    let rc = serde_json::from_str::<VerificationMethod>(&json);
    assert!(rc.is_ok());

    let vm2 = rc.unwrap();
    assert_eq!(vm, vm2);

    let vmr = vm.to_reference();
    assert_eq!(vmr.is_reference(), true);
    assert_eq!(vm.id(), vmr.id());
    assert_ne!(vm, vmr);
}

#[test]
fn test_verification_method_serde_reference() {
    let controller = Id::random();
    let id = format!("{}#key-1", controller.to_did_string());
    let vm = VerificationMethod::reference(id.clone());
    assert_eq!(vm.is_reference(), true);
    assert_eq!(vm.id(), &id);
    assert_eq!(vm.method_type(), None);
    assert_eq!(vm.controller(), None);

    let json = serde_json::to_string(&vm).unwrap();
    println!("reference json - {}", json);
    println!("vm: {}", vm);

    let rc = serde_json::from_str::<VerificationMethod>(&json);
    assert!(rc.is_ok());
    let vm2 = rc.unwrap();
    assert_eq!(vm, vm2);

    let mvr = vm.to_reference();
    assert!(mvr.is_reference());
    assert_eq!(vm, mvr);
}

#[test]
fn test_verification_method_default_entity() {
    let controller = Id::random();
    let id = format!("{}#key-1", controller.to_did_string());

    let vm = VerificationMethod::entity(
        id.clone(),
        VerificationMethodType::Ed25519VerificationKey2020,
        controller.clone(),
        controller.to_base58()
    );
    assert_eq!(vm.is_reference(), false);
    assert_eq!(vm.id(), &id);
    assert_eq!(vm.method_type(), Some(VerificationMethodType::Ed25519VerificationKey2020));
    assert_eq!(vm.controller(), Some(&controller));
    assert_eq!(vm.public_key_multibase(), Some(controller.to_base58().as_str()));

    let mut vmr = VerificationMethod::reference(id.clone());
    assert_eq!(vmr.is_reference(), true);
    assert_eq!(vmr.id(), &id);
    assert_eq!(vmr.method_type(), None);
    assert_eq!(vmr.controller(), None);
    assert_eq!(vmr.public_key_multibase(), None);

    if let VerificationMethod::Entity(vm) = vm.clone() {
        vmr.update_reference(vm.clone()).unwrap();
        assert_eq!(vm.method_type(), vmr.method_type());
        assert_eq!(vm.controller(), vmr.controller());
        assert_eq!(vm.public_key_multibase(), vmr.public_key_multibase());

        let json = serde_json::to_string(&vmr).unwrap();
        println!("updated reference json - {}", json);
        println!("vmr: {}", vmr);
        let rc = serde_json::from_str::<VerificationMethod>(&json);
        assert!(rc.is_ok());

        let vmr2 = rc.unwrap();
        assert_eq!(vmr, vmr2);
    }
}
