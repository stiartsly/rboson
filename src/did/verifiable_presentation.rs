
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

use crate::{
    Id,
    error::{Error, Result},
    core::crypto_identity::CryptoIdentity,
};

use super::{
    DIDUrl,
    Proof,
    VerifiableCredential,
    VerifiablePresentationBuilder,
    Vouch
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiablePresentation {
    #[serde(rename = "@context", skip_serializing_if = "Vec::is_empty")]
    contexts: Vec<String>,

    #[serde(rename = "id", skip_serializing_if = "super::is_none_or_empty")]
    id: Option<String>,

    #[serde(rename = "type", skip_serializing_if = "Vec::is_empty")]
    types: Vec<String>,

    #[serde(rename = "holder")]
    holder: Id,

    #[serde(rename = "verifiableCredential", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<VerifiableCredential>,

    #[serde(rename = "proof", skip_serializing_if = "Option::is_none")]
    proof: Option<Proof>
}

impl VerifiablePresentation {
    pub(crate) fn unsigned(
        _context: Vec<String>,
        _id: String,
        _types: Vec<String>,
        _subject: String,
        _credentials: HashMap<String, VerifiableCredential>,
    ) -> Self {
        unimplemented!()
    }

    pub(crate) fn signed(mut _unsigned: Self, _proof: Option<Proof>) -> Self {
        unimplemented!()
    }

    pub fn from_vouch(_vouch: &Vouch) -> Self{
        unimplemented!()
    }

    pub fn contexts(&self) -> Vec<&str> {
        self.contexts.iter().map(|c| c.as_str()).collect()
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn types(&self) -> Vec<&str> {
        self.types.iter().map(|t| t.as_str()).collect()
    }

    pub fn holder(&self) -> &Id {
        &self.holder
    }

    pub fn credentials(&self) -> Vec<&VerifiableCredential> {
        self.credentials.iter().collect()
    }

    pub fn credential_by_type(&self, credential_type: &str) -> Option<&VerifiableCredential> {
        self.credentials.iter().find(|vc|
            vc.types().iter().any(|t| t == &credential_type)
        )
    }

    pub fn credential_by_id(&self, _id: &str) -> Option<&VerifiableCredential> {
        unimplemented!()
    }

    pub fn credential_by_didurl(&self, id: &DIDUrl) -> Option<&VerifiableCredential> {
        self.credentials.iter().find(|vc|
            vc.id() == id.to_string().as_str()
        )
    }

    pub fn proof(&self) -> Option<&Proof> {
        self.proof.as_ref()
    }

    pub fn validate(&self) -> Result<()> {
         match self.is_geniune() {
            true => Ok(()),
            false => Err(Error::Signature("VP signature is not valid".to_string())),
        }
    }

    pub fn is_geniune(&self) -> bool {
        self.proof.as_ref().map(|v|v.verify(
            self.holder(),
            &self.to_sign_data()
        )).unwrap_or(false)
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        //BosonVouch::unsigned(self.clone()).to_sign_data()
        unimplemented!()
    }

    pub fn builder(holder: CryptoIdentity) -> VerifiablePresentationBuilder {
        VerifiablePresentationBuilder::new(holder)
    }
}

impl TryFrom<&str> for VerifiablePresentation {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e|
            Error::Argument(format!("Failed to parse VP from string: {}", e))
        )
    }
}

impl TryFrom<&[u8]> for VerifiablePresentation {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|e|
            Error::Argument(format!("Failed to parse VP from bytes: {}", e))
        )
    }
}

impl From<&VerifiablePresentation> for String {
    fn from(vp: &VerifiablePresentation) -> Self {
        serde_json::to_string(&vp).unwrap()
    }
}

impl From<&VerifiablePresentation> for Vec<u8> {
    fn from(vp: &VerifiablePresentation) -> Self {
        serde_json::to_vec(&vp).unwrap()
    }
}

impl From<&VerifiablePresentation> for Vouch {
    fn from(_vp: &VerifiablePresentation) -> Self {
        unimplemented!()
    }
}

impl From<&Vouch> for VerifiablePresentation {
    fn from(vouch: &Vouch) -> Self {
        Self::from_vouch(&vouch)
    }
}

impl Hash for VerifiablePresentation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.contexts.hash(state);
        self.types.hash(state);
        for credential in self.credentials.iter() {
            credential.hash(state);
        }
    }
}

/*
pub(crate) struct BosonVouch {
    vouch: Vouch,
    vp: Option<VerifiablePresentation>,
}

impl BosonVouch {
    fn unsigned(vp: VerifiablePresentation) -> Self {
        let vouch = Vouch::unsigned(
                vp.id().as_ref()
                    .and_then(|id| DIDUrl::parse(id).ok().and_then(|did| did.fragment().map(|frag| frag.to_string())))
                    .unwrap_or_else(|| "".to_string()),
                vp.types().iter()
                    .filter(|t| *t == &did_constants::DEFAULT_VP_TYPE)
                    .map(|t| t.to_string())
                    .collect(),
                vp.holder().clone(),
                vp.credentials().iter().map(|c| Credential::from(*c)).collect()
        );

        Self {
            vouch,
            vp: Some(vp),
        }
    }

    fn to_sign_data(&self) -> Vec<u8> {
        self.vouch.to_sign_data()
*/