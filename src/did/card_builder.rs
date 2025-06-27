use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde::Serialize;
use serde_json::Map;

use crate::{
    Error,
    error::Result,
    core::crypto_identity::CryptoIdentity,
};

use super::{
    Card,
    card::Service,
    Credential,
    CredentialBuilder,
    BosonIdentityObjectBuilder
};

pub struct CardBuilder {
    identity    : CryptoIdentity,
    credentials : Vec<Credential>,
    services    : Vec<Service>,
}

impl CardBuilder {
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        Self {
            identity,
            credentials: Vec::new(),
            services: Vec::new(),
        }
    }

    pub fn with_credential(&mut self, credential: Credential) -> Result<&mut Self> {
        if credential.subject().id() != self.identity.id() {
            return Err(Error::Argument("Credential subject does not match identity".into()));
        }
        self.credentials.push(credential);
        Ok(self)
    }

    pub fn with_credentials(&mut self, credentials: Vec<Credential>) -> Result<&mut Self> {
        for credential in &credentials {
            if credential.subject().id() != self.identity.id() {
                return Err(Error::Argument("The subject of one Credential does not match identity".into()));
            }
        }
        self.credentials.extend(credentials);
        Ok(self)
    }

    pub fn add_credentials_by_claims<T>(&mut self,
        id: &str,
        credential_type: &str,
        claims: HashMap<String, T>
    ) -> Result<&mut Self>
        where T: Serialize {

        self.with_credential(CredentialBuilder::new(self.identity.clone())
            .with_id(id)
            .with_types(&[credential_type])
            .with_claims(claims)
            .build()?
        )
    }

    pub fn add_service<T>(&mut self,
        id: String,
        service_type: String,
        endpoint: String,
        properties: HashMap<String, T>
    ) -> Result<&mut Self>
        where T: Serialize {

        if properties.keys().any(|key|
            key == "id" || key == "t" || key == "e"
        ) {
            return Err(Error::Argument("Service properties cannot contain 'id', 't' or 'e'".into()));
        }

        let mut map = Map::new();
        for (k, v) in properties {
            map.insert(
                k.nfc().collect::<String>(),
                Self::normalize(serde_json::to_value(v).unwrap())
            );
        }

        self.services.push(
            Service::new(id, service_type, endpoint, map)
        );
        Ok(self)
    }

    pub fn build(&self) -> Result<Card> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for CardBuilder {
    type BosonIdentityObject = Card;

    fn identity(&self) -> &CryptoIdentity {
        &self.identity
    }

    fn build(&self) -> Result<Self::BosonIdentityObject> {
        let unsigned = Card::unsigned(
            self.identity.id().clone(),
            self.credentials.clone(),
            self.services.clone(),
        );

        let data = unsigned.to_sign_data();
        Ok(Card::signed(
            unsigned,
            Some(Self::now()),
            Some(data)
        ))
    }
}
