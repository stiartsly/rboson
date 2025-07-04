use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

use crate::{
    Id,
    Error,
    CryptoIdentity,
    core::Result,
};

use crate::did::{
    did_constants as constants,
    DIDUrl,
    Proof,
    Vouch,
    w3c::{
        VerifiableCredential as VC,
        VerifiablePresentationBuilder,
    }
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiablePresentation {
    #[serde(rename = "@context", skip_serializing_if = "Vec::is_empty")]
    contexts: Vec<String>,

    #[serde(rename = "id", skip_serializing_if = "crate::did::is_none_or_empty")]
    id: Option<String>,

    #[serde(rename = "type", skip_serializing_if = "Vec::is_empty")]
    types: Vec<String>,

    #[serde(rename = "holder")]
    holder: Id,

    #[serde(rename = "verifiableCredential", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<VC>,

    #[serde(rename = "proof", skip_serializing_if = "Option::is_none")]
    proof: Option<Proof>
}

impl VerifiablePresentation {
    pub(crate) fn unsigned(
        contexts: Vec<String>,
        id: Option<String>,
        types: Vec<String>,
        holder: Id,
        credentials: Vec<VC>,
    ) -> Self {
        Self {
            contexts,
            id,
            types,
            holder,
            credentials,
            proof: None,
        }
    }

    pub(crate) fn signed(mut unsigned: Self, proof: Option<Proof>) -> Self {
        unsigned .proof = proof;
        unsigned
    }

    pub fn from_vouch(vouch: &Vouch) -> Self {
        Self::from_vouch_with_type_contexts(
            vouch,
            HashMap::new()
        )
    }

    pub fn from_vouch_with_type_contexts(
        vouch: &Vouch,
        type_contexts: HashMap<String, Vec<String>>
    ) -> Self{
        if let Some(vp) = vouch.vp() {
            return vp.clone();
        }

        let mut types = vec![
            constants::DEFAULT_VP_TYPE
        ];
        let mut contexts = vec![
            constants::W3C_VC_CONTEXT,
            constants::BOSON_VC_CONTEXT,
            constants::W3C_ED25519_CONTEXT
        ];

        for t in vouch.types() {
            if t == constants::DEFAULT_VP_TYPE {
                continue;
            }
            types.push(t);

            if let Some(ctxts) = type_contexts.get(t) {
                ctxts.iter().for_each(|c| {
                    if !contexts.contains(&c.as_str()) {
                        contexts.push(c.as_str());
                    }
                });
            }
        }

        Self {
            contexts    : contexts.iter().map(|c| c.to_string()).collect(),
            id          : vouch.id().map(|id| id.to_string()),
            types       : types.iter().map(|t| t.to_string()).collect(),
            holder      : vouch.holder().clone(),
            credentials : vouch.credentials().iter()
                            .map(|c| VC::from(*c))
                            .collect(),
            proof       : None, // unsigned VP has no proof
        }
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

    pub fn credentials(&self) -> Vec<&VC> {
        self.credentials.iter().collect()
    }

    pub fn credential_by_type(&self, credential_type: &str) -> Option<&VC> {
        self.credentials.iter().find(|vc|
            vc.types().iter().any(|t| t == &credential_type)
        )
    }

    pub fn credential_by_id(&self, _id: &str) -> Option<&VC> {
        unimplemented!()
    }

    pub fn credential_by_didurl(&self, id: &DIDUrl) -> Option<&VC> {
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
        unimplemented!()
    }

    pub(crate) fn to_vouch(&self) -> Vouch {
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
    fn from(vp: &VerifiablePresentation) -> Self {
        vp.to_vouch()
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