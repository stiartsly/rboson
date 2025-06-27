
use std::time::SystemTime;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde_json::{Map, Value};

use crate::{
    Id,
    error::{Error, Result},
    core::crypto_identity::CryptoIdentity,
    Identity,
};

use super::{
    did_constants,
    BosonIdentityObjectBuilder,
    VerifiableCredential,
    DIDUrl,
    Proof,
    proof::{ProofType, ProofPurpose},
    VerificationMethod,
};

pub struct VerifiableCredentialBuilder {
    issuer      : CryptoIdentity,
    contexts    : Vec<String>,
    id          : Option<String>,
    types       : Vec<String>,
    name        : Option<String>,
    description : Option<String>,
    valid_from  : Option<SystemTime>,
    valid_until : Option<SystemTime>,
    subject     : Option<Id>,
    claims      : Map<String, Value>,
}

impl VerifiableCredentialBuilder {
    pub(crate) fn new(issuer: CryptoIdentity) -> Self {
        Self {
            issuer,
            contexts    : Vec::new(),
            id          : None,
            types       : Vec::new(),
            name        : None,
            description : None,
            valid_from  : None,
            valid_until : None,
            subject     : None,
            claims      : Map::new(),
        }
    }

    pub fn with_id(&mut self, id: &str) -> &mut Self {
        if id.is_empty() {
            return self;
        }

        let scheme = format!("{}:", did_constants::DID_SCHEME);
        if id.starts_with(&scheme) {
            let Ok(uri) = DIDUrl::parse(id) else {
                return self;
            };

            if uri.fragment().is_none() {
                return self;
            }
        }

        self.id = Some(id.nfc().collect::<String>());
        self
    }

    pub fn with_types(&mut self, credential_type: &str,  contexts: Vec<String>) -> &mut Self {
        if credential_type.is_empty() {
            return self;
        }

        let type_ = credential_type.nfc().collect::<String>();
        if !self.types.contains(&type_) {
            self.types.push(type_);
        }

        for ctx in contexts {
            if ctx.is_empty() {
                continue;
            }

            let ctx = ctx.nfc().collect::<String>();
            if self.contexts.contains(&ctx) {
                continue;
            }
            self.types.push(ctx);
        }

        self
    }

    pub fn with_name(&mut self, name: &str) -> &mut Self {
        if name.is_empty() {
            return self;
        }

        self.name = Some(
            name.nfc().collect::<String>()
        );
        self
    }

    pub fn with_description(&mut self, description: &str) -> &mut Self {
        if description.is_empty() {
            return self;
        }

        self.description = Some(
            description.nfc().collect::<String>()
        );
        self
    }

    pub fn with_valid_from(&mut self, valid_from: SystemTime) -> &mut Self {
        self.valid_from = Some(Self::trim_millis(valid_from));
        self
    }

    pub fn with_valid_until(&mut self, valid_until: SystemTime) -> &mut Self {
        self.valid_until = Some(Self::trim_millis(valid_until));
        self
    }

    pub fn with_subject(&mut self, subject: Id) -> &mut Self {
        self.subject = Some(subject);
        self
    }

    pub fn with_claims<T>(&mut self, claims: HashMap<String, T>) -> &mut Self
        where T: serde::Serialize {
        if claims.is_empty() {
            return self;
        }
        let mut values_map = Map::new();
        for (k, v) in claims {
            if k.is_empty() {
                continue;
            }

            let value = serde_json::to_value(v).unwrap();
            let key = k.nfc().collect::<String>();
            let val = Self::normalize(value);
            values_map.insert(key, val);
        }

        self.claims = values_map;
        self
    }

    pub fn add_claims<T>(&mut self, claims: HashMap<String, T>) -> &mut Self
    where T: serde::Serialize {
        if claims.is_empty() {
            return self;
        }

        for (k, v) in claims {
            if k.is_empty() {
                continue;
            }

            let value = serde_json::to_value(v).unwrap();
            let key = k.nfc().collect::<String>();
            let val = Self::normalize(value);

            if !self.claims.contains_key(&key) {
                self.claims.insert(key, val);
            }
        }
        self
    }

    pub fn build(&self) -> Result<VerifiableCredential> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for VerifiableCredentialBuilder {
    type BosonIdentityObject = VerifiableCredential;

    fn identity(&self) -> &CryptoIdentity {
        &self.issuer
    }

    fn build(&self) -> Result<VerifiableCredential> {
        let Some(id) = self.id.as_deref() else {
            return Err(Error::Argument("Credenital Id is missing".into()));
        };

        let did_url;
        let scheme = did_constants::DID_SCHEME;
        if id.starts_with(scheme) {
            let Ok(url) = DIDUrl::parse(id) else {
                return Err(Error::Argument(format!("Invalid specs to create DIDUrl: {}", id)));
            };
            if url.id() != self.issuer.id() {
                return Err(Error::State(format!("Invalid credential id: should be the subject id based DIDURL: {}", url)));
            }
            if url.fragment().is_none() {
                return Err(Error::State(format!("Invalid credential id: must contain the fragment part: {}", url)));
            }
            did_url = Some(url);
        } else {
            did_url = Some(DIDUrl::new(
                self.subject.as_ref().unwrap(),
                None,
                None,
                Some(id)
            ));
        }

        if self.claims.is_empty() {
            return Err(Error::Argument("Missing claims for the credential".into()));
        }

        let unsigned = VerifiableCredential::unsigned(
            self.contexts.clone(),
            did_url.unwrap().to_string(),
            self.types.clone(),
            self.name.clone(),
            self.description.clone(),
            self.issuer.id().clone(),
            self.valid_from,
            self.valid_until,
            self.subject.clone(),
            self.claims.clone(),
        );

        let data = unsigned.to_sign_data();
        let signature = self.identity().sign_into(&data).unwrap();
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            VerifiableCredentialBuilder::now(),
            VerificationMethod::default_reference(self.identity().id()),
            ProofPurpose::AssertionMethod,
            signature
        );

        Ok(VerifiableCredential::signed(
            unsigned,
            proof,
        ))
    }
}
