use std::collections::HashMap;
use serde::Serialize;

use crate::{
    Error,
    error::Result,
    Identity,
    CryptoIdentity,
};

use crate::did::{
    did_constants,
    BosonIdentityObjectBuilder,
    VerificationMethod,
    proof::{Proof, ProofType, ProofPurpose},
    w3c::{
        DIDDocument,
        VerifiableCredential,
        VerifiableCredentialBuilder,
        did_document::Service,
    }
};

pub struct DIDDocumentBuilder {
    identity            : CryptoIdentity,
    contexts            : Vec<String>,
    verification_methods: HashMap<String, VerificationMethod>,
    authentications     : HashMap<String, VerificationMethod>,
    assertions          : HashMap<String, VerificationMethod>,
    credentials         : HashMap<String, VerifiableCredential>,
    services            : HashMap<String, Service>
}

impl DIDDocumentBuilder {
    pub fn new(identity: CryptoIdentity) -> Self {

        let contexts: Vec<String> = vec![
            did_constants::W3C_DID_CONTEXT.into(),
            did_constants::BOSON_DID_CONTEXT.into(),
            did_constants::W3C_ED25519_CONTEXT.into()
        ];
        let def_method = VerificationMethod::default_entity(identity.id().clone());
        let verification_methods = HashMap::from([
            (def_method.id().to_string(), def_method.clone())
        ]);

        /*let def_method_ref = def_method.to_reference();
        let authentications = HashMap::from([
            (def_method_ref.id().to_string(), def_method.clone())
        ]);*/

        Self {
            identity,
            contexts,
            verification_methods,
            authentications     : HashMap::new(),
            assertions          : HashMap::new(),
            credentials         : HashMap::new(),
            services            : HashMap::new(),
        }
    }

    pub fn with_credential(&mut self, vc: VerifiableCredential) -> Result<&mut Self> {
        if vc.subject().id() != self.identity.id() {
            return Err(Error::Argument("VC subject does not match identity".into()));
        }
        self.credentials.insert(vc.id().into(), vc);
        Ok(self)
    }

    pub fn with_credentials(&mut self, vcs: Vec<VerifiableCredential>) -> Result<&mut Self> {
        for vc in &vcs {
            if vc.subject().id() != self.identity.id() {
                return Err(Error::Argument("The subject of one VC does not match identity".into()));
            }
        }
        for vc in vcs {
            self.credentials.insert(vc.id().into(), vc);
        }
        Ok(self)
    }

    pub fn add_credentials_by_claims<T>(&mut self,
        id: &str,
        credential_type: &str,
        contexts: Vec<String>,
        claims: HashMap<String, T>
    ) -> Result<&mut Self>
        where T: Serialize {

        self.with_credential(VerifiableCredentialBuilder::new(self.identity.clone())
            .with_id(id)
            .with_types(credential_type, contexts)
            .with_claims(claims)
            .build()?
        )
    }

    pub fn add_service<T>(&mut self,
        _id: String,
        _service_type: String,
        _endpoint: String,
        _properties: HashMap<String, T>
    ) -> Result<&mut Self>
        where T: Serialize {

        unimplemented!()
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
            self.authentications.values().cloned().collect(),
            self.assertions.values().cloned().collect(),
            self.credentials.values().cloned().collect(),
            self.services.values().cloned().collect(),
        );

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        let def_method = VerificationMethod::default_entity(self.identity.id().clone());

        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            Self::now(),
            def_method, // TODO
            ProofPurpose::AssertionMethod,
            signature
        );
        Ok(DIDDocument::signed(
            unsigned,
            Some(proof)
        ))
    }
}
