
use std::fmt;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

use crate::{
    Id,
    Error,
    core::Result
};

use crate::did::{
    did_constants,
    DIDUrl
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
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

impl VerificationMethod {
    pub fn entity(id: &str,
        method_type : VerificationMethodType,
        controller  : &Id,
        public_key_multibase: String
    ) -> Self {
        Self::Entity(Entity {
            id          : id.into(),
            method_type : Some(method_type),
            controller  : Some(controller.clone()),
            public_key_multibase: Some(public_key_multibase),
        })
    }

    pub(crate) fn reference(id: &str) -> Self {
        Self::Reference(Reference {
            id      : id.into(),
            entity  : None,
        })
    }

    pub(crate) fn default_entity(id: &Id) -> Self {
        Self::entity(
            &Self::to_default_method_id(id),
            VerificationMethodType::Ed25519VerificationKey2020,
            id,
            id.to_base58(),
        )
    }

    pub(crate) fn default_reference(id: &Id) -> Self {
        Self::reference(&Self::to_default_method_id(id))
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

    pub fn to_reference(&self) -> VerificationMethod {
        match self {
            VerificationMethod::Reference(_) => self.clone(),
            VerificationMethod::Entity(v) => VerificationMethod::Reference(
                Reference::from_entity(v.clone())
            )
        }
    }

    #[allow(unused)]
    pub(crate) fn update_reference(&mut self, entity: Entity) -> Result<()> {
        match self {
            VerificationMethod::Reference(ref mut r) => r.update_reference(entity),
            VerificationMethod::Entity(_) => Err(Error::Argument("Cannot update Entity with Reference".into())),
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
            write!(f, "type={},controller:{},publicKeyMultibase:{}}}",
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

    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    method_type: Option<VerificationMethodType>,

    #[serde(rename = "controller", skip_serializing_if = "Option::is_none")]
    #[serde(with="crate::serde_option_id_as_base58")]
    controller: Option<Id>,

    #[serde(rename = "publicKeyMultibase", skip_serializing_if = "crate::is_none_or_empty")]
    public_key_multibase: Option<String>
}

impl Entity {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn method_type(&self) -> Option<VerificationMethodType> {
        self.method_type
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
    pub(crate) fn from_entity(entity: Entity) -> Self {
        Reference {
            id: entity.id.clone(),
            entity: Some(entity),
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn method_type(&self) -> Option<VerificationMethodType> {
        self.entity.as_ref().and_then(|v| v.method_type())
    }

    pub(crate) fn controller(&self) -> Option<&Id> {
        self.entity.as_ref().and_then(|v| v.controller())
    }

    pub(crate) fn public_key_multibase(&self) -> Option<&str> {
        self.entity.as_ref().and_then(|v| v.public_key_multibase.as_deref())
    }

    pub(crate) fn is_reference(&self) -> bool {
        true
    }

    pub(crate) fn update_reference(&mut self, entity: Entity) -> Result<()>{
        if entity.is_reference() {
            return Err(Error::Argument("Cannot update Reference with another Reference".into()));
        }

        if entity.id != self.id {
            return Err(Error::Argument("Entity ID does not match Reference ID".into()));
        }
        self.entity = Some(entity);
        Ok(())
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
