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

use boson_api::{
    Result,
    errors::ArgumentError,
    CryptoIdentity,
    Identity
};

use crate::{
    did_constants as constants,
    proof::{Proof, ProofPurpose, ProofType},
    w3c::{VerifiableCredential, VerifiablePresentation as VP},
    BosonIdentityObjectBuilder,
    VerificationMethod,
};

pub struct VerifiablePresentationBuilder {
    holder: CryptoIdentity,

    contexts: Vec<String>,
    id: Option<String>,
    types: Vec<String>,
    credentials: IndexMap<String, VerifiableCredential>,
}

impl VerifiablePresentationBuilder {
    pub(crate) fn new(holder: CryptoIdentity) -> Self {
        let types = vec![constants::DEFAULT_VP_TYPE]
            .iter()
            .map(|s| s.nfc().collect::<String>())
            .collect();

        let contexts = vec![
            constants::W3C_VC_CONTEXT,
            constants::BOSON_VC_CONTEXT,
            constants::W3C_ED25519_CONTEXT,
        ]
        .iter()
        .map(|s| s.nfc().collect::<String>())
        .collect();

        Self {
            holder,
            contexts,
            id: None,
            types,
            credentials: IndexMap::new(),
        }
    }

    pub fn with_id(&mut self, id: &str) -> Result<&mut Self> {
        if id.is_empty() {
            self.id = None;
            return Ok(self);
        }
        self.id = Some(Self::normalize_subject_id(self.holder.id(), id)?.to_string());
        Ok(self)
    }

    pub fn with_types(&mut self, credential_type: &str, contexts: Vec<&str>) -> Result<&mut Self> {
        if credential_type.is_empty() {
            return Err(ArgumentError::new("Credential type cannot be empty"));
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
            if !self.contexts.contains(&ctx) {
                self.contexts.push(ctx);
            }
        }
        Ok(self)
    }

    pub fn with_credential(&mut self, vc: VerifiableCredential) -> &mut Self {
        self.credentials.insert(vc.id().to_string(), vc);
        self
    }

    pub fn with_credentials(&mut self, vcs: HashMap<&str, VerifiableCredential>) -> &mut Self {
        for (k, vc) in vcs {
            self.credentials.insert(k.to_string(), vc);
        }
        self
    }

    pub fn with_credential_by_claims<T>(
        &mut self,
        id: &str,
        credential_type: &str,
        contexts: Vec<&str>,
        claims: HashMap<&str, T>,
    ) -> Result<&mut Self>
    where
        T: serde::Serialize,
    {
        if id.is_empty() {
            return Err(ArgumentError::new("Credential Id cannot be empty"));
        }

        if credential_type.is_empty() {
            return Err(ArgumentError::new("Credential type cannot be empty"));
        }

        let vc = VerifiableCredential::builder(self.holder.clone())
            .with_id(id)?
            .with_types(credential_type, contexts)?
            .with_claims(claims)
            .build()?;

        self.with_credential(vc);
        Ok(self)
    }

    pub fn build(&self) -> Result<VP> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for VerifiablePresentationBuilder {
    type BosonIdentityObject = VP;

    fn identity(&self) -> &CryptoIdentity {
        &self.holder
    }

    fn build(&self) -> Result<Self::BosonIdentityObject> {
        if self.credentials.is_empty() {
            Err(ArgumentError::new("VCs can not empty for VP"))?
        }

        let unsigned = VP::unsigned(
            self.contexts.clone(),
            self.id.clone(),
            self.types.clone(),
            self.holder.id().clone(),
            self.credentials.values().cloned().collect(),
        );

        let signed_at = Self::now();
        let view = unsigned.to_unsigned_boson_vouch().stamp_signed_at(signed_at);
        let signature = self.holder.sign_into(&view.to_sign_data())?;
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            signed_at,
            VerificationMethod::default_reference(self.identity().id()),
            ProofPurpose::AssertionMethod,
            signature,
        );

        Ok(VP::signed(unsigned, Some(proof)))
    }
}
