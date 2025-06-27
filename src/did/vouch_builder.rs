
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::{
    error::{Error, Result},
    core::crypto_identity::CryptoIdentity,
};

use super::{
    BosonIdentityObjectBuilder,
    Credential,
    Vouch,
};

pub struct VouchBuilder {
    identity    : CryptoIdentity,
    id          : Option<String>,
    types       : Vec<String>,
    credentials : HashMap<String, Credential>,
}

impl VouchBuilder {
    pub fn new(holder: &CryptoIdentity) -> Self {
        Self {
            identity: holder.clone(),
            id: None,
            types: Vec::new(),
            credentials: HashMap::new(),
        }
    }

    pub fn with_id(&mut self, id: &str) -> &mut Self {
        self.id = Some(id.nfc().collect::<String>());
        self
    }

    pub fn with_types(&mut self, types: Vec<String>) -> &mut Self {
        for t in types {
            if t.is_empty() {
                continue;
            }

            let normalized = t.nfc().collect::<String>();
            if self.types.iter().any(|v| v == &normalized) {
                continue;
            }

            self.types.push(normalized);
        }
        self
    }

    pub fn add_crendetial(&mut self, credential: Credential) -> &mut Self {
        self.credentials.insert(credential.id().to_string(), credential);
        self
    }

    pub fn add_credentials(&mut self, credentials: Vec<Credential>) -> &mut Self {
        for credential in credentials {
            self.add_crendetial(credential);
        }
        self
    }

    pub fn build(&self) -> Result<Vouch> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for VouchBuilder {
    type BosonIdentityObject = Vouch;

    fn identity(&self) -> &CryptoIdentity {
        &self.identity
    }

    fn build(&self) -> Result<Self::BosonIdentityObject> {
        if self.id.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
            return Err(Error::Argument("Id cannot be empty".into()));
        }

        if self.credentials.is_empty() {
            return Err(Error::Argument("Claims cannot be empty".into()));
        }

        let unsigned = Vouch::unsigned(
            self.id.as_ref().unwrap().clone(),
            self.types.clone(),
            self.identity.id().clone(),
            self.credentials.values().cloned().collect()
        );

        let data = unsigned.to_sign_data();
        Ok(Vouch::signed(
            unsigned,
            Some(Self::now()),
            Some(data)
        ))
    }
}
