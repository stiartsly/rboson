use std::time::SystemTime;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde_json::{Map, Value};

use crate::{
    Id,
    error::{Error, Result},
    core::crypto_identity::CryptoIdentity,
};

use super::{
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
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        Self {
            identity,
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
        match !id.is_empty() {
            true => self.id = Some(id.nfc().collect::<String>()),
            false => {},
        }
        self
    }

    pub fn with_types(&mut self, types: &[&str]) -> &mut Self {
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

    pub fn add_types(&mut self, types: &[&str]) -> &mut Self {
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

        let name = name.nfc().collect::<String>();
        self.name = Some(name);
        self
    }

    pub fn with_description(&mut self, description: &str) -> &mut Self {
        if description.is_empty() {
            return self;
        }

        let descr = description.nfc().collect::<String>();
        self.description = Some(descr);
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
            let value = serde_json::to_value(v).unwrap();
            let key = k.nfc().collect::<String>();
            let val = Self::normalize(value);

            if !self.claims.contains_key(&key) {
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
        let unsigned = Credential::unsigned(
            self.id.as_ref().unwrap().clone(),
            self.types.clone(),
            self.name.clone(),
            self.description.clone(),
            Some(id.clone()),
            self.valid_from,
            self.valid_until,
            self.subject.as_ref().map(|s| s.clone()).unwrap_or(id),
            self.claims.clone()
        );

        let data = unsigned.to_sign_data();
        Ok(Credential::signed(
            unsigned,
            Some(Self::now()),
            Some(data)
        ))
    }
}
