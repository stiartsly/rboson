
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::{
    error::{Error, Result},
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
    types       : Option<Vec<String>>,
    credentials : HashMap<String, Credential>,
}

impl VouchBuilder {
    pub(crate) fn new(holder: CryptoIdentity) -> Self {
        Self {
            identity: holder,
            id: None,
            types: None,
            credentials: HashMap::new(),
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

        if self.types.is_none() {
            self.types = Some(Vec::new());
        }

        let types = self.types.as_mut().unwrap();
        let t = credential_type.nfc().collect::<String>();
        if !types.contains(&t) {
            types.push(t);
        }
        self
    }

    pub fn with_types(&mut self, types: Vec<&str>) -> &mut Self {
        for t in types {
            if t.is_empty() {
                continue;
            }

            if self.types.is_none() {
                self.types = Some(Vec::new());
            }
            let types = self.types.as_mut().unwrap();
            let t = t.nfc().collect::<String>();
            if types.contains(&t) {
                continue;
            }
            types.push(t);
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

        self.with_credential(
            Credential::builder(self.identity.clone())
                .with_id(id)
                .with_type(credential_type)
                .with_claims(claims)
                .build()?
        );
        Ok(self)
    }

    pub fn with_credential(&mut self, credential: Credential) -> &mut Self {
        self.credentials.insert(credential.id().to_string(), credential);
        self
    }

    pub fn with_credentials(&mut self, credentials: Vec<Credential>) -> &mut Self {
        for credential in credentials {
            self.with_credential(credential);
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
            return Err(Error::Argument("Claims cannot be empty".into()));
        }

        let unsigned = Vouch::unsigned(
            self.id.clone(),
            self.types.clone(),
            self.identity.id().clone(),
            self.credentials.values().cloned().collect()
        );

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        Ok(Vouch::signed(
            unsigned,
            Some(Self::now()),
            Some(signature)
        ))
    }
}
