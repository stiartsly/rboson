use boson::{
    did::{VerificationMethod, VerificationMethodType},
    Id,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_method_deserializes_entity_object() {
        let json = "{\
            \"id\":\"did:boson:D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv#key-1\",\
            \"type\":\"Ed25519VerificationKey2020\",\
            \"controller\":\"did:boson:D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv\",\
            \"publicKeyMultibase\":\"D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv\"\
        }";

        let vm: VerificationMethod = serde_json::from_str(json).unwrap();
        assert!(!vm.is_reference());

        let controller = Id::try_from("D43CcXZPpR81qA1eBTfmALNvXpswLp3gMDPUsowjGtSv").unwrap();
        let id = format!("{}#key-1", controller.to_did_string());
        assert_eq!(vm.id(), &id);
        assert_eq!(
            vm.method_type(),
            Some(VerificationMethodType::Ed25519VerificationKey2020)
        );
        assert_eq!(vm.controller(), Some(&controller));
        assert_eq!(
            vm.public_key_multibase(),
            Some(controller.to_base58().as_str())
        );

        let wire = serde_json::to_value(&vm).unwrap();
        assert!(wire.is_object());
        assert_eq!(wire["id"], id);
        assert_eq!(wire["controller"], controller.to_did_string());
    }

    #[test]
    fn test_verification_method_deserializes_reference_string() {
        let id = "did:boson:4K6hgZPEAcQT1hLkADbapDAoPhnfhbWUtcEYseQ1i8as#key-1";
        let vm: VerificationMethod = serde_json::from_str(&format!("\"{id}\"")).unwrap();

        assert!(vm.is_reference());
        assert_eq!(vm.id(), id);
        assert_eq!(vm.method_type(), None);
        assert_eq!(vm.controller(), None);

        assert_eq!(serde_json::to_value(&vm).unwrap(), serde_json::json!(id));
    }

    #[test]
    fn test_verification_method_reference_serializes_as_id() {
        let controller = Id::random();
        let id = format!("{}#key-1", controller.to_did_string());
        let vm = VerificationMethod::entity(
            &id,
            VerificationMethodType::Ed25519VerificationKey2020,
            &controller,
            controller.to_base58(),
        );
        let reference = vm.to_reference();

        assert!(reference.is_reference());
        assert_eq!(reference.id(), id);
        assert_eq!(
            serde_json::to_value(&reference).unwrap(),
            serde_json::json!(id)
        );
    }
}
