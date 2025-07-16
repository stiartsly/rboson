
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::{
    Error,
    CryptoIdentity,
    core::Result,
};

use crate::did::{
    did_constants as constants,
    BosonIdentityObjectBuilder,
    DIDUrl,
    VerificationMethod,
    proof::{Proof, ProofType, ProofPurpose},
    w3c::{
        VerifiableCredential,
        VerifiablePresentation as VP,
    }
};

pub struct VerifiablePresentationBuilder {
    holder      : CryptoIdentity,

    contexts    : Vec<String>,
    id          : Option<String>,
    types       : Vec<String>,
    credentials : HashMap<String, VerifiableCredential>,
}

impl VerifiablePresentationBuilder {
    pub(crate) fn new(holder: CryptoIdentity) -> Self {
        let types = vec![
            constants::DEFAULT_VP_TYPE
        ].iter().map(|s|
            s.nfc().collect::<String>()
        ).collect();

        let contexts = vec![
            constants::W3C_VC_CONTEXT,
            constants::BOSON_VC_CONTEXT,
            constants::W3C_ED25519_CONTEXT
        ].iter().map(|s|
            s.nfc().collect::<String>()
        ).collect();

        Self {
            holder,
            contexts,
            id          : None,
            types,
            credentials : HashMap::new(),
        }
    }

    pub fn with_id(&mut self, id: &str) -> Result<&mut Self> {
        if id.is_empty() {
            return Err(Error::Argument("Credential Id cannot be empty".into()));
        }

        let did_url = if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            let url = DIDUrl::parse(&id).map_err(|_| {
                Error::Argument(format!("Id must has the fragment part: {}", id))
            })?;
            if url.fragment().is_none() {
                Err(Error::Argument("Id must has the fragment part".into()))?;
            }
            url
        } else {
            DIDUrl::new(
                self.holder.id(),
                None,
                None,
                Some(id)
            )
        };

        self.id = Some(did_url.to_string());
        Ok(self)
    }

    pub fn with_types(
        &mut self,
        credential_type: &str,
        contexts: Vec<&str>
    ) -> Result<&mut Self> {
        if credential_type.is_empty() {
            return Err(Error::Argument("Credential type cannot be empty".into()));
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

    pub fn with_credentials(
        &mut self,
        vcs: HashMap<&str, VerifiableCredential>
    ) -> &mut Self {
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
        claims: HashMap<&str, T>
    ) -> Result<&mut Self>
    where
        T: serde::Serialize,
    {
        if id.is_empty() {
            return Err(Error::Argument("Credential Id cannot be empty".into()));
        }

        if credential_type.is_empty() {
            return Err(Error::Argument("Credential type cannot be empty".into()));
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
            Err(Error::Argument("VCs can not empty for VP".into()))?
        }

        let unsigned = VP::unsigned(
            self.contexts.clone(),
            self.id.clone(),
            self.types.clone(),
            self.holder.id().clone(),
            self.credentials.values().cloned().collect(),
        );

        let signature = self.holder.sign_into(&unsigned.to_sign_data())?;
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            Self::now(),
            VerificationMethod::default_reference(self.identity().id()),
            ProofPurpose::AssertionMethod,
            signature
        );

        Ok(VP::signed(
            unsigned,
            Some(proof)
        ))
    }
}
