use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use serde_json::Map;

use crate::{
    Error,
    CryptoIdentity,
    core::Result,
};

use crate::did::{
    Card,
    card::Service,
    Credential,
    BosonIdentityObjectBuilder
};

pub struct CardBuilder {
    identity    : CryptoIdentity,
    credentials : HashMap<String, Credential>,
    services    : HashMap<String, Service>,
}

impl CardBuilder {
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        Self {
            identity,
            credentials : HashMap::new(),
            services    : HashMap::new(),
        }
    }

    pub fn with_credential(&mut self, credential: Credential) -> Result<&mut Self> {
        if credential.subject().id() != self.identity.id() {
            Err(Error::Argument("Credential subject does not match identity".into()))?;
        }

        self.credentials.insert(credential.id().to_string(), credential);
        Ok(self)
    }

    pub fn with_credentials(&mut self, credentials: Vec<Credential>) -> Result<&mut Self> {
        if credentials.is_empty() {
            Err(Error::Argument("Credentials cannot be empty".into()))?;
        }
        for credential in &credentials {
            if credential.subject().id() != self.identity.id() {
                Err(Error::Argument("The subject of one Credential does not match identity".into()))?;
            }
        }

        for credential in credentials {
            self.credentials.insert(credential.id().to_string(), credential);
        }
        Ok(self)
    }

    pub fn with_credential_by_claims<T>(&mut self,
        id: &str,
        credential_type: &str,
        claims: HashMap<&str, T>
    ) -> Result<&mut Self>
    where T: serde::Serialize
    {
        if claims.is_empty() {
            Err(Error::Argument("Claims cannot be empty".into()))?;
        }
        self.with_credential(
            Credential::builder(self.identity.clone())
                .with_id(id)
                .with_type(credential_type)
                .with_claims(claims)
                .build()?
        )
    }

    pub fn with_service<T>(&mut self,
        id: &str,
        service_type: &str,
        endpoint: &str,
        properties: HashMap<&str, T>
    ) -> Result<&mut Self>
        where T: serde::Serialize
    {
        if id.is_empty() || service_type.is_empty() || endpoint.is_empty() {
            Err(Error::Argument("Service id, type and endpoint cannot be empty".into()))?;
        }

        let mut map = Map::new();
        for (k, v) in properties {
            map.insert(
                k.nfc().collect::<String>(),
                Self::normalize(serde_json::to_value(v).unwrap())
            );
        }

        self.services.insert(
            id.to_string(),
            Service::new(
                id.to_string(),
                service_type.to_string(),
                endpoint.to_string(),
                map
            )
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
        let creds = match self.credentials.is_empty() {
            true => None,
            false => Some(self.credentials.values().cloned().collect()),
        };
        let services = match self.services.is_empty() {
            true => None,
            false => Some(self.services.values().cloned().collect()),
        };

        let unsigned = Card::unsigned(
            self.identity.id().clone(),
            creds,
            services,
            None,
        );

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        Ok(Card::signed(
            unsigned,
            Some(Self::now()),
            Some(signature)
        ))
    }
}
