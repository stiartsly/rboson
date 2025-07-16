use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    unwrap,
    Error,
    core::Result,
    CryptoIdentity,
};

use crate::did::{
    did_constants as constants,
    BosonIdentityObjectBuilder,
    VerificationMethod as VM,
    proof::{Proof, ProofType, ProofPurpose},
    DIDUrl,
    w3c::{
        DIDDocument,
        VerifiableCredential as VC,
        diddoc::Service,
    }
};

pub struct DIDDocumentBuilder {
    identity            : CryptoIdentity,
    contexts            : Vec<String>,
    verification_methods: HashMap<String, VM>,
    authentications     : Vec<VM>,
    assertions          : Vec<VM>,
    credentials         : Vec<VC>,
    services            : HashMap<String, Service>,

    def_method_ref      : Option<VM>
}

impl DIDDocumentBuilder {
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        let contexts: Vec<String> = vec![
            constants::W3C_DID_CONTEXT,
            constants::BOSON_DID_CONTEXT,
            constants::W3C_ED25519_CONTEXT
        ].iter().map(|s| s.to_string()).collect();

        let mut builder = Self {
            identity,
            contexts,
            verification_methods: HashMap::new(),
            authentications     : Vec::new(),
            assertions          : Vec::new(),
            credentials         : Vec::new(),
            services            : HashMap::new(),
            def_method_ref      : None,
        };

        let def_method = VM::default_entity(builder.identity().id());
        let def_method_ref = def_method.to_reference();


        builder.with_verification_method(def_method)
            .with_authentication(def_method_ref.clone())
            .with_assertion(def_method_ref.clone());

        builder.def_method_ref = Some(def_method_ref);
        builder
    }

    pub fn with_context(&mut self, context: &str) -> Result<&mut Self> {
        if context.is_empty() {
            Err(Error::Argument("Context cannot be empty".into()))?;
        }
        let normalized = context.nfc().collect::<String>();
        if !self.contexts.contains(&normalized) {
            self.contexts.push(normalized);
        }
        Ok(self)
    }

    pub fn with_contexts(&mut self, contexts: Vec<&str>) -> Result<&mut Self> {
        if contexts.is_empty() {
            Err(Error::Argument("Contexts cannot be empty".into()))?;
        }
        for context in contexts {
            if context.is_empty() {
                continue;
            }
            let normalized = context.nfc().collect::<String>();
            if !self.contexts.contains(&normalized) {
                self.contexts.push(normalized);
            }
        }
        Ok(self)
    }

    fn with_verification_method(&mut self, vm: VM) -> &mut Self {
        self.verification_methods.insert(vm.id().to_string(), vm);
        self
    }

    fn with_authentication(&mut self, mut vm: VM) -> &mut Self {
        if vm.is_reference() {
            let Some(VM::Entity(existing)) = self.verification_methods.get(vm.id()) else {
                return self;
            };
            _ = vm.update_reference(existing.clone());
            self.authentications.push(vm);
        } else {
            self.with_authentication(vm.clone());
            self.authentications.push(vm);
        }
        self
    }

    fn with_assertion(&mut self, mut vm: VM) -> &mut Self {
        if vm.is_reference() {
            let Some(VM::Entity(existing)) = self.verification_methods.get(vm.id()) else {
                return self;
            };
            _ = vm.update_reference(existing.clone());
            self.assertions.push(vm);
        } else {
            self.with_assertion(vm.clone());
            self.assertions.push(vm);
        }
        self
    }

    pub fn with_credential(&mut self, vc: VC) -> Result<&mut Self> {
        if vc.subject().id() != self.identity.id() {
            Err(Error::Argument("VC subject does not match identity".into()))?;
        }
        self.credentials.push(vc);
        Ok(self)
    }

    pub fn with_credentials(&mut self, vcs: Vec<VC>) -> Result<&mut Self> {
        for vc in &vcs {
            if vc.subject().id() != self.identity.id() {
                Err(Error::Argument("The subject of one VC does not match identity".into()))?;
            }
            self.credentials.push(vc.clone());
        }
        Ok(self)
    }

    pub fn with_credential_by_claims<T>(&mut self,
        id: &str,
        credential_type: &str,
        contexts: Vec<&str>,
        claims: HashMap<&str, T>
    ) -> Result<&mut Self>
        where T: Serialize {

        if credential_type.is_empty() {
            Err(Error::Argument("Credential type cannot be empty".into()))?;
        }

        self.with_credential(VC::builder(self.identity.clone())
            .with_id(id)?
            .with_types(credential_type, contexts)?
            .with_claims(claims)
            .build()?
        )
    }

    pub fn with_service<T>(&mut self,
        id: &str,
        service_type: &str,
        endpoint: &str,
        properties: HashMap<&str, T>
    ) -> Result<&mut Self>
        where T: Serialize + DeserializeOwned {

        let did_url = if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            let url = id.parse::<DIDUrl>()
                .map_err(|_| Error::Argument("Invalid DID URL format".into()))?;
            if url.id() != Some(self.identity.id()) {
                Err(Error::Argument("DID URL id does not match subject id".into()))?;
            }
            if url.fragment().is_none() {
                Err(Error::Argument("DID URL must have a fragment part".into()))?;
            }
            url
        } else {
            DIDUrl::new(self.identity.id(), None, None, Some(id))
        };

        if service_type.is_empty() {
            Err(Error::Argument("Service type cannot be empty".into()))?;
        }
        if endpoint.is_empty() {
            Err(Error::Argument("Service endpoint cannot be empty".into()))?;
        }

        if properties.contains_key("id") ||
           properties.contains_key("type") ||
           properties.contains_key("serviceEndpoint") {
            Err(Error::Argument("Service properties cannot contain 'id', 'type' or 'serviceEndpoint'".into()))?;
        }

        let properties = properties.iter().filter_map(|(k, v)| {
                serde_json::to_value(v)
                    .ok()
                    .map(|value| (k.to_string(), value))
            }).collect::<serde_json::Map<String, serde_json::Value>>();

        let canonical_id = did_url.to_string();
        let service = Service::new(
            canonical_id.clone(),
            service_type.to_string(),
            endpoint.to_string(),
            properties
        );

        self.services.insert(canonical_id, service);
        Ok(self)
    }

    pub fn build(&self) -> Result<DIDDocument> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for DIDDocumentBuilder {
    type BosonIdentityObject = DIDDocument;

    fn identity(&self) -> &CryptoIdentity {
        &self.identity
    }

    fn build(&self) -> Result<Self::BosonIdentityObject> {
        let unsigned = DIDDocument::unsigned(
            self.contexts.clone(),
            self.identity.id().clone(),
            self.verification_methods.values().cloned().collect(),
            self.authentications.clone(),
            self.assertions.clone(),
            self.credentials.clone(),
            self.services.values().cloned().collect(),
        );

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            Self::now(),
            unwrap!(self.def_method_ref).clone(),
            ProofPurpose::AssertionMethod,
            signature
        );
        Ok(DIDDocument::signed(
            unsigned,
            Some(proof)
        ))
    }
}
