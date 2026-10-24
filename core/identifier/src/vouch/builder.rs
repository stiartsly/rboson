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

use indexmap::IndexMap;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::{BosonIdentityObjectBuilder, Credential, Vouch};
use boson_api::{errors::ArgumentError, CryptoIdentity, Identity, Result};

pub struct VouchBuilder {
    identity: CryptoIdentity,
    id: Option<String>,
    types: Vec<String>,
    credentials: IndexMap<String, Credential>,
}

impl VouchBuilder {
    pub(crate) fn new(holder: CryptoIdentity) -> Self {
        Self {
            identity: holder,
            id: None,
            types: Vec::new(),
            credentials: IndexMap::new(),
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
            return Err(ArgumentError::new("Claims cannot be empty"));
        }

        let credential = Credential::builder(self.identity.clone())
            .with_id(id)
            .with_type(credential_type)
            .with_claims(claims)
            .build()?;
        self.credentials
            .insert(credential.id().to_string(), credential);
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
            return Err(ArgumentError::new("Credentials cannot be empty"));
        }

        let types = match self.types.is_empty() {
            true => None,
            false => Some(self.types.clone()),
        };
        let signed_at = Self::now();
        let unsigned = Vouch::unsigned(
            self.id.clone(),
            types,
            self.identity.id().clone(),
            self.credentials.values().cloned().collect(),
            None,
        )
        .stamp_signed_at(signed_at);

        let signature = self.identity.sign_into(&unsigned.to_sign_data())?;
        Ok(Vouch::signed(unsigned, None, Some(signature)))
    }
}
