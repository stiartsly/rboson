
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::{
    error::{Error, Result},
    core::crypto_identity::CryptoIdentity,
    Identity,
};

use super::{
    did_constants,
    BosonIdentityObjectBuilder,
    VerifiableCredential,
    DIDUrl,
    proof::{Proof, ProofType, ProofPurpose},
    VerificationMethod,
    VerifiablePresentation,
};

#[allow(unused)]
pub struct VerifiablePresentationBuilder {
    holder      : CryptoIdentity,

    contexts    : Vec<String>,
    id          : Option<String>,
    types       : Vec<String>,
    credentials : HashMap<String, VerifiableCredential>,
}

#[allow(unused)]
impl VerifiablePresentationBuilder {
    pub(crate) fn new(holder: CryptoIdentity) -> Self {
        let types = vec![
            did_constants::DEFAULT_VP_TYPE.into(),
            did_constants::W3C_VC_CONTEXT.into(),
            did_constants::BOSON_VC_CONTEXT.into(),
            did_constants::W3C_ED25519_CONTEXT.into()
        ];

        Self {
            holder,
            contexts: Vec::new(),
            id: None,
            types: types,
            credentials: HashMap::new(),
        }
    }

    pub fn with_id(&mut self, id: &str) -> &mut Self {
        if id.is_empty() {
            return self;
        }

        let scheme = format!("{}:", did_constants::DID_SCHEME);
        if id.starts_with(&scheme) {
            let Ok(uri) = DIDUrl::parse(id) else {
                return self;
            };

            if uri.fragment().is_none() {
                return self;
            }
        }

        self.id = Some(id.nfc().collect::<String>());
        self
    }

    pub fn with_types(&mut self, credential_type: &str,  contexts: Vec<String>) -> &mut Self {
        if credential_type.is_empty() {
            return self;
        }

        let type_ = credential_type.nfc().collect::<String>();
        if !self.types.contains(&type_) {
            self.types.push(type_);
        }

        for ctx in contexts {
            if ctx.is_empty() {
                continue;
            }

            let ctx = ctx.nfc().collect::<String>();
            if self.contexts.contains(&ctx) {
                continue;
            }
            self.types.push(ctx);
        }

        self
    }

    pub fn add_credential(&mut self, _credential: VerifiableCredential) -> &mut Self {
        unimplemented!()
    }

    pub fn with_credentials(&mut self, credenitals: HashMap<String, VerifiableCredential>) -> &mut Self {
        if credenitals.is_empty() {
            return self;
        }
        unimplemented!()
    }

    pub fn build(&self) -> Result<VerifiablePresentation> {
        BosonIdentityObjectBuilder::build(self)
    }
}

impl BosonIdentityObjectBuilder for VerifiablePresentationBuilder {
    type BosonIdentityObject = VerifiablePresentation;

    fn identity(&self) -> &CryptoIdentity {
        &self.holder
    }

    fn build(&self) -> Result<VerifiablePresentation> {
        let Some(id) = self.id.as_deref() else {
            return Err(Error::Argument("Missing credenital Id".into()));
        };

        let _did_url;
        let scheme = did_constants::DID_SCHEME;
        if id.starts_with(scheme) {
            let Ok(url) = DIDUrl::parse(id) else {
                return Err(Error::Argument(format!("Invalid specs to create DIDUrl: {}", id)));
            };
            if url.id() != self.holder.id() {
                return Err(Error::State(format!("Id must be the holder id based DIDURL: {}", url)));
            }
            if url.fragment().is_none() {
                return Err(Error::State(format!("Id must has the fragment part: {}", url)));
            }
            _did_url = Some(url);
        } else {
            _did_url = Some(DIDUrl::new(
                self.holder.id(),
                None,
                None,
                Some(id)
            ));
        }

        if self.credentials.is_empty() {
            return Err(Error::Argument("Missing credential for VerifiablePresentation".into()));
        }

        let unsigned = VerifiablePresentation::unsigned(
            self.contexts.clone(),
            self.id.as_ref().unwrap().clone(),
            self.types.clone(),
            self.id.clone().unwrap(),
            self.credentials.clone(),
        );

        let signature = self.holder.sign_into(&unsigned.to_sign_data())?;
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            Self::now(),
            VerificationMethod::default_reference(self.identity().id()),
            ProofPurpose::AssertionMethod,
            signature
        );

        Ok(VerifiablePresentation::signed(
            unsigned,
            Some(proof)
        ))
    }
}
