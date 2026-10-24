// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::collections::HashMap;
use indexmap::IndexMap;
use serde_json::Map;
use unicode_normalization::UnicodeNormalization;

use boson_api::{
    errors::ArgumentError,
    CryptoIdentity,
    Identity,
    Result
};
use crate::{
    card::Service,
    BosonIdentityObjectBuilder,
    Card,
    Credential
};

pub struct CardBuilder {
    identity: CryptoIdentity,
    credentials: IndexMap<String, Credential>,
    services: IndexMap<String, Service>,
}

impl CardBuilder {
    pub(crate) fn new(identity: CryptoIdentity) -> Self {
        Self {
            identity,
            credentials: IndexMap::new(),
            services: IndexMap::new(),
        }
    }

    pub fn with_credential(&mut self, credential: Credential) -> Result<&mut Self> {
        if credential.subject().id() != self.identity.id() {
            Err(ArgumentError::new(
                "Credential subject does not match identity",
            ))?;
        }

        self.credentials
            .insert(credential.id().to_string(), credential);
        Ok(self)
    }

    pub fn with_credentials(&mut self, credentials: Vec<Credential>) -> Result<&mut Self> {
        if credentials.is_empty() {
            return Ok(self);
        }
        for credential in &credentials {
            if credential.subject().id() != self.identity.id() {
                Err(ArgumentError::new(
                    "The subject of one Credential does not match identity",
                ))?;
            }
        }

        for credential in credentials {
            self.credentials
                .insert(credential.id().to_string(), credential);
        }
        Ok(self)
    }

    pub fn with_credential_by_claims<T>(
        &mut self,
        id: &str,
        credential_type: &str,
        claims: HashMap<&str, T>,
    ) -> Result<&mut Self>
    where
        T: serde::Serialize,
    {
        if claims.is_empty() {
            Err(ArgumentError::new("Claims cannot be empty"))?;
        }
        self.with_credential(
            Credential::builder(self.identity.clone())
                .with_id(id)
                .with_type(credential_type)
                .with_claims(claims)
                .build()?,
        )
    }

    pub fn with_service<T>(
        &mut self,
        id: &str,
        service_type: &str,
        endpoint: &str,
        properties: HashMap<&str, T>,
    ) -> Result<&mut Self>
    where
        T: serde::Serialize,
    {
        if id.is_empty() || service_type.is_empty() || endpoint.is_empty() {
            Err(ArgumentError::new(
                "Service id, type and endpoint cannot be empty",
            ))?;
        }
        if properties
            .keys()
            .any(|key| matches!(*key, "id" | "t" | "e"))
        {
            Err(ArgumentError::new(
                "Service properties must not contain reserved keys: id, t, e",
            ))?;
        }

        let mut map = Map::new();
        for (k, v) in properties {
            map.insert(
                k.nfc().collect::<String>(),
                Self::normalize(serde_json::to_value(v).unwrap()),
            );
        }

        self.services.insert(
            id.to_string(),
            Service::new(
                id.nfc().collect(),
                service_type.nfc().collect(),
                endpoint.nfc().collect(),
                map,
            ),
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

        let unsigned = Card::unsigned(self.identity.id().clone(), creds, services, None);

        let signed_at = Some(Self::now());
        let unsigned = Card::signed(unsigned, signed_at, None);
        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        Ok(Card::signed(unsigned, signed_at, Some(signature)))
    }
}
