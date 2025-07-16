
use std::time::SystemTime;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde_json::{Map, Value};

use crate::{
    Id,
    error::{Error, Result},
    CryptoIdentity,
};

use crate::did::{
    constants,
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
        let types: Vec<String> = vec![
            constants::DEFAULT_VC_TYPE
        ].iter().map(|s|
            s.nfc().collect::<String>()
        ).collect();

        let contexts: Vec<String> = vec![
            constants::W3C_VC_CONTEXT,
            constants::BOSON_VC_CONTEXT,
            constants::W3C_ED25519_CONTEXT
        ].iter().map(|s|
            s.nfc().collect::<String>()
        ).collect();

        Self {
            issuer,
            contexts,
            id          : None,
            types,
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
            Err(Error::Argument("Credential Id cannot be empty".into()))?;
        }

        if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            let url = id.parse::<DIDUrl>().map_err(|_| {
                Error::Argument(format!("Id must has the fragment part: {}", id))
            })?;

            if url.fragment().is_none() {
                Err(Error::Argument("Id must has the fragment part".into()))?;
            }

            /*
            if url.id() != self.issuer.id() {
                Err(Error::Argument(format!(
                    "Invalid credential id: should be the subject id based DIDURL: {}",
                    url
                )))?;
            }*/
        }

        self.id = Some(id.nfc().collect::<String>());
        Ok(self)
    }

    pub fn with_type(
        &mut self,
        credential_type: &str,
        context: &str
    ) -> Result<&mut Self> {
        if credential_type.is_empty() {
            Err(Error::Argument("Credential type cannot be empty".into()))?;
        }

        let t = credential_type.nfc().collect::<String>();
        if !self.types.contains(&t) {
            self.types.push(t);
        }

        if context.is_empty() {
            return Ok(self);
        }

        let ctx = context.nfc().collect::<String>();
        if !self.contexts.contains(&ctx) {
            self.contexts.push(ctx);
        }

        Ok(self)
    }

    pub fn with_types(
        &mut self,
        credential_type: &str,
        contexts: Vec<&str>
    ) -> Result<&mut Self> {
        if credential_type.is_empty() {
            Err(Error::Argument("Credential type cannot be empty".into()))?;
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
            self.contexts.push(ctx);
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
            self.claims.insert(
                key,
                Self::normalize(serde_json::to_value(value).unwrap())
            );
        }
        self
    }

    pub fn with_claims<T>(&mut self, claims: HashMap<&str, T>) -> &mut Self
    where T: serde::Serialize
    {
        if claims.is_empty() {
            return self;
        }

        for (k, v) in claims {
            let key = k.nfc().collect::<String>();
            if !self.claims.contains_key(&key) {
                self.claims.insert(
                    key,
                    Self::normalize(serde_json::to_value(v).unwrap())
                );
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

    fn build(&self) -> Result<Self::BosonIdentityObject> {
        if self.id.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
            Err(Error::Argument("VC Id cannot be empty".into()))?;
        }
        let Some(id) = self.id.as_ref() else {
            return Err(Error::Argument("VC Id cannot be empty".into()));
        };
        if self.claims.is_empty() {
            return Err(Error::Argument("VC Claims can not be empty".into()));
        }

        let did_url = if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            let url = id.parse::<DIDUrl>()?;
            if url.fragment().is_none() {
                return Err(Error::Argument("VC Id must has the fragment part".into()));
            }
            url
        } else {
            DIDUrl::new(
                self.subject.as_ref().unwrap_or(self.issuer.id()),
                None,
                None,
                Some(id)
            )
        };

        let unsigned = Self::BosonIdentityObject::unsigned(
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

        Ok(Self::BosonIdentityObject::signed(
            unsigned,
            proof,
        ))
    }
}
