
use std::time::SystemTime;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde_json::{Map, Value};

use crate::{
    Id,
    error::{Error, Result},
    CryptoIdentity,
    Identity,
};

use crate::did::{
    did_constants::DID_SCHEME as did_scheme,
    DIDUrl,
    Proof,
    proof::{ProofType, ProofPurpose},
    VerificationMethod,
    BosonIdentityObjectBuilder,
    w3c::VerifiableCredential,
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

    pub fn with_id(&mut self, id: &str) -> Result<&mut Self> {
        if id.is_empty() {
            return Err(Error::Argument("Credential Id cannot be empty".into()));
        }

        let id = id.nfc().collect::<String>();
        if id.starts_with(did_scheme) {
            let url = DIDUrl::parse(&id).map_err(|_| {
                Error::Argument(format!("Id must has the fragment part: {}", id))
            })?;

            if url.id() != self.issuer.id() {
                return Err(Error::Argument(format!(
                    "Invalid credential id: should be the subject id based DIDURL: {}",
                    url
                )));
            }
        }

        self.id = Some(id);
        Ok(self)
    }

    pub fn with_types(
        &mut self,
        credential_type: &str,
        contexts: Vec<String>
    ) -> Result<&mut Self> {
        if credential_type.is_empty() {
            return Err(Error::Argument("Credential type cannot be empty".into()));
        }

        let t = credential_type.nfc().collect::<String>();
        if !self.types.contains(&t) {
            self.types.push(t);
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
        Ok(self)
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

    pub fn with_claim<T>(&mut self, name: &str, value: T) -> &mut Self
    where T: serde::Serialize
    {
        if name.is_empty() {
            return self;
        }

        let key = name.nfc().collect::<String>();
        if !self.claims.contains_key(&key) {
            let val = Self::normalize(serde_json::to_value(value).unwrap());
            self.claims.insert(key, val);
        }
        self
    }

    pub fn with_claims<T>(&mut self, claims: HashMap<String, T>) -> &mut Self
    where T: serde::Serialize
    {
        if claims.is_empty() {
            return self;
        }

        for (k, v) in claims {
            let key = k.nfc().collect::<String>();
            if !self.claims.contains_key(&key) {
                let val = Self::normalize(serde_json::to_value(v).unwrap());
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
        if self.id.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
            return Err(Error::Argument("VC Id cannot be empty".into()));
        }
        let Some(id) = self.id.as_ref() else {
            return Err(Error::Argument("VC Id cannot be empty".into()));
        };

        let did_url: DIDUrl;
        if id.starts_with(did_scheme) {
            did_url = DIDUrl::parse(id).unwrap();
        } else {
            did_url = DIDUrl::new(
                self.subject.as_ref().unwrap_or(self.issuer.id()),
                None,
                None,
                Some(id)
            );
        }

        if self.claims.is_empty() {
            return Err(Error::Argument("Missing claims for the credential".into()));
        }

        let unsigned = VerifiableCredential::unsigned(
            self.contexts.clone(),
            did_url.to_string(),
            self.types.clone(),
            self.name.clone(),
            self.description.clone(),
            self.issuer.id().clone(),
            self.valid_from,
            self.valid_until,
            self.subject.clone(),
            self.claims.clone(),
        );

        let signature = self.identity().sign_into(&unsigned.to_sign_data())?;
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
