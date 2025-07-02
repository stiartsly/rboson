use std::time::SystemTime;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde_json::{Map, Value};

use crate::{
    as_secs,
    Id,
    error::{Error, Result},
    CryptoIdentity,
    Identity,
};

use crate::did::{
    Credential,
    BosonIdentityObjectBuilder
};

pub struct CredentialBuilder {
    identity    : CryptoIdentity,
    id          : Option<String>,
    types       : Vec<String>,
    name        : Option<String>,
    description : Option<String>,
    valid_from  : Option<SystemTime>,
    valid_until : Option<SystemTime>,
    subject     : Option<Id>,
    claims      : Map<String, Value>,
}

impl CredentialBuilder {
    pub(crate) fn new(issuer: CryptoIdentity) -> Self {
        Self {
            identity    : issuer,
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
        if !id.is_empty() {
            self.id = Some(id.nfc().collect::<String>());
        }
        self
    }

    pub fn with_type(&mut self, credential_type: &str) -> &mut Self {
        if credential_type.is_empty() {
            return self;
        }

        let t = credential_type.nfc().collect::<String>();
        if !self.types.contains(&t) {
            self.types.push(t);
        }
        self
    }

    pub fn with_types(&mut self, types: Vec<&str>) -> &mut Self {
        for t in types {
            if t.is_empty() {
                continue;
            }

            let t = t.nfc().collect::<String>();
            if self.types.contains(&t) {
                continue;
            }
            self.types.push(t);
        }
        self
    }

    pub fn with_name(&mut self, name: &str) -> &mut Self {
        if name.is_empty() {
            return self;
        }
        self.name = Some(name.nfc().collect::<String>());
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
        where T: serde::Serialize {
        let key = name.nfc().collect::<String>();
        if !self.claims.contains_key(&key) {
            let val = Self::normalize(serde_json::to_value(value).unwrap());
            self.claims.insert(key, val);
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
                let val = Self::normalize(serde_json::to_value(v).unwrap());
                self.claims.insert(key, val);
            }
        }
        self
    }

    pub fn build(&self) -> Result<Credential> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for CredentialBuilder {
    type BosonIdentityObject = Credential;

    fn identity(&self) -> &CryptoIdentity {
        &self.identity
    }

    fn build(&self) -> Result<Self::BosonIdentityObject> {
        if self.id.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
            return Err(Error::Argument("Id cannot be empty".into()));
        }
        if self.claims.is_empty() {
            return Err(Error::Argument("Claims cannot be empty".into()));
        }

        let id = self.identity.id().clone();
        let types = match self.types.is_empty() {
            true => None,
            false => Some(self.types.clone()),
        };
        let unsigned = Credential::unsigned(
            self.id.as_ref().unwrap().clone(),
            types,
            self.name.clone(),
            self.description.clone(),
            Some(id.clone()),
            self.valid_from.as_ref().map(|v| as_secs!(v)),
            self.valid_until.as_ref().map(|v| as_secs!(v)),
            self.subject.as_ref().map(|s| s.clone()).unwrap_or(id),
            self.claims.clone(),
            None,
        );

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        Ok(Credential::signed(
            unsigned,
            Some(as_secs!(Self::now())),
            Some(signature)
        ))
    }
}
