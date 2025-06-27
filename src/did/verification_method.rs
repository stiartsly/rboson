
use std::fmt;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

use crate::Id;
use super::{
    did_url::DIDUrl,
    did_constants
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub enum VerificationMethodType {
    Ed25519VerificationKey2020,
}

impl fmt::Display for VerificationMethodType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VerificationMethodType::Ed25519VerificationKey2020 => write!(f, "Ed25519VerificationKey2020"),
        }
    }
}

#[derive(Debug, Clone, Eq)]
#[derive(Serialize, Deserialize)]
pub enum VerificationMethod {
    Reference(Reference),
    Entity(Entity),
}

#[allow(unused)]
impl VerificationMethod {
    pub fn entity(id: String,
        method_type: VerificationMethodType,
        controller: Id,
        public_key_multibase: String
    ) -> Self {
        VerificationMethod::Entity(Entity {
            id,
            method_type,
            controller: Some(controller),
            public_key_multibase: Some(public_key_multibase),
        })
    }

    pub fn reference(id: String) -> Self {
        VerificationMethod::Reference(Reference {
            id      : id.into(),
            entity  : None,
        })
    }

    pub fn default_entity(id: &Id) -> Self {
        Self::entity(
            Self::to_default_method_id(id),
            VerificationMethodType::Ed25519VerificationKey2020,
            id.clone(),
            id.to_base58(),
        )
    }

    pub fn default_reference(id: &Id) -> Self {
        Self::reference(Self::to_default_method_id(id))
    }

    fn to_default_method_id(id: &Id) -> String {
        DIDUrl::new(
            id,
            None,
            None,
            Some(did_constants::DEFAULT_VERIFICATION_METHOD_FRAGMENT)
        ).to_string()
    }

    pub fn id(&self) -> &str {
        match self {
            VerificationMethod::Reference(v) => v.id(),
            VerificationMethod::Entity(v) => v.id(),
        }
    }

    pub fn method_type(&self) -> Option<VerificationMethodType> {
        match self {
            VerificationMethod::Reference(v) => v.method_type(),
            VerificationMethod::Entity(v) => v.method_type(),
        }
    }

    pub fn controller(&self) -> Option<&Id> {
        match self {
            VerificationMethod::Entity(v) => v.controller(),
            VerificationMethod::Reference(v) => v.controller(),
        }
    }

    pub fn public_key_multibase(&self) -> Option<&str> {
        match self {
            VerificationMethod::Entity(v) => v.public_key_multibase(),
            VerificationMethod::Reference(v) => v.public_key_multibase(),
        }
    }

    pub fn is_reference(&self) -> bool {
        match self {
            VerificationMethod::Entity(v) => v.is_reference(),
            VerificationMethod::Reference(v) => v.is_reference(),
        }
    }

    pub fn to_reference(&self) -> Option<Reference> {
        match self.is_reference() {
            true => match self {
                VerificationMethod::Reference(v) => Some(v.clone()),
                VerificationMethod::Entity(_) => None,
            },
            false => None,
        }
    }
}

impl Hash for VerificationMethod {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            VerificationMethod::Reference(v) => v.hash(state),
            VerificationMethod::Entity(v) => v.hash(state),
        }
    }
}

impl PartialEq for VerificationMethod {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VerificationMethod::Reference(a), VerificationMethod::Reference(b)) => a == b,
            (VerificationMethod::Entity(a), VerificationMethod::Entity(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for VerificationMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VerificationMethod{{id:{}\'", self.id())?;
        if !self.is_reference() {
            write!(f, "type={}, controller:{}, publicKeyMultibase:{}}}",
                self.method_type().unwrap(),
                self.controller().unwrap().to_did_string(),
                self.public_key_multibase().unwrap()
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
pub struct Entity {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "type")]
    method_type: VerificationMethodType,

    #[serde(rename = "controller", skip_serializing_if = "Option::is_none")]
    controller: Option<Id>,

    #[serde(rename = "publicKeyMultibase", skip_serializing_if = "super::is_none_or_empty")]
    public_key_multibase: Option<String>
}

impl Entity {
    pub(crate) fn id(&self) -> &str {
        unimplemented!()
    }

    pub(crate) fn method_type(&self) -> Option<VerificationMethodType> {
        unimplemented!()
    }

    pub(crate) fn controller(&self) -> Option<&Id> {
        self.controller.as_ref()
    }

    pub(crate) fn public_key_multibase(&self) -> Option<&str> {
        self.public_key_multibase.as_deref()
    }

    pub(crate) fn is_reference(&self) -> bool {
        false
    }
}

impl Hash for Entity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.method_type.hash(state);
        if let Some(controller) = &self.controller {
            controller.hash(state);
        }
        if let Some(public_key) = &self.public_key_multibase {
            public_key.hash(state);
        }
    }
}

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct Reference {
    id      : String,
    entity  : Option<Entity>,
}

impl Reference {
    pub(crate) fn id(&self) -> &str {
        unimplemented!()
    }

    pub(crate) fn method_type(&self) -> Option<VerificationMethodType> {
        unimplemented!()
    }

    pub(crate) fn controller(&self) -> Option<&Id> {
        self.entity.as_ref().and_then(|e| e.controller())
    }

    pub(crate) fn public_key_multibase(&self) -> Option<&str> {
        self.entity.as_ref().and_then(|e| e.public_key_multibase.as_deref())
    }

    pub(crate) fn is_reference(&self) -> bool {
        true
    }
}

impl Hash for Reference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for Reference {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
