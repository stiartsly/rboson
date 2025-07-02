
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::{
    core::{Error, Result},
    CryptoIdentity,
    Identity,
};

use crate::did::{
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
    pub(crate) fn new(holder: CryptoIdentity) -> Self {
        Self {
            identity    : holder,
            id          : None,
            types       : Vec::new(),
            credentials : HashMap::new(),
        }
    }

    pub fn with_id(&mut self, id: &str) -> &mut Self {
        self.id = Some(id.nfc().collect::<String>());
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
            if !self.types.contains(&t) {
                self.types.push(t);
            }
        }
        self
    }

    pub fn with_credential_by_claims<T>(&mut self,
        id: &str,
        credential_type: &str,
        claims: HashMap<&str, T>
    ) -> Result<&mut Self>
    where T: serde::Serialize {

        if claims.is_empty() {
            return Err(Error::Argument("Claims cannot be empty".into()));
        }

        self.credentials.insert(
            id.to_string(),
            Credential::builder(self.identity.clone())
                .with_id(id)
                .with_type(credential_type)
                .with_claims(claims)
                .build()?
        );
        Ok(self)
    }

    pub fn with_credential(&mut self, cred: Credential) -> &mut Self {
        self.credentials.insert(cred.id().to_string(), cred);
        self
    }

    pub fn with_credentials(&mut self, credentials: Vec<Credential>) -> &mut Self {
        for cred in credentials {
            self.credentials.insert(cred.id().to_string(), cred);
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
        if self.credentials.is_empty() {
            return Err(Error::Argument("Credentials cannot be empty".into()));
        }

        let types = match self.types.is_empty() {
            true => None,
            false => Some(self.types.clone()),
        };
        let unsigned = Vouch::unsigned(
            self.id.clone(),
            types,
            self.identity.id().clone(),
            self.credentials.values().cloned().collect(),
            None,
        );

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        Ok(Vouch::signed(
            unsigned,
            Some(Self::now()),
            Some(signature)
        ))
    }
}
