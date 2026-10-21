use serde::de::{MapAccess, Visitor};
use serde::de::value::MapAccessDeserializer;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::{
    errors::ArgumentError, Result,
    utils::codec,
    Id
};
use super::{did_constants, DIDUrl};

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerificationMethodType {
    Ed25519VerificationKey2020,
}

impl fmt::Display for VerificationMethodType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VerificationMethodType::Ed25519VerificationKey2020 => {
                write!(f, "Ed25519VerificationKey2020")
            }
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub enum VerificationMethod {
    Reference(Reference),
    Entity(Entity),
}

impl VerificationMethod {
    pub fn entity(
        id: &str,
        method_type: VerificationMethodType,
        controller: &Id,
        public_key_multibase: String,
    ) -> Self {
        Self::Entity(Entity {
            id: id.into(),
            method_type: Some(method_type),
            controller: Some(controller.clone()),
            public_key_multibase: Some(public_key_multibase),
        })
    }

    pub fn reference(id: &str) -> Self {
        Self::Reference(Reference {
            id: id.into(),
            entity: None,
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
            Some(did_constants::DEFAULT_VERIFICATION_METHOD_FRAGMENT),
        )
        .to_string()
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
            VerificationMethod::Entity(v) => {
                VerificationMethod::Reference(Reference::from_entity(v.clone()))
            }
        }
    }

    #[allow(unused)]
    pub(crate) fn update_reference(&mut self, entity: Entity) -> Result<()> {
        match self {
            VerificationMethod::Reference(ref mut r) => r.update_reference(entity),
            VerificationMethod::Entity(_) => {
                Err(ArgumentError::new("Cannot update Entity with Reference").into())
            }
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

impl Serialize for VerificationMethod {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Reference(reference) => serializer.serialize_str(reference.id()),
            Self::Entity(entity) => entity.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for VerificationMethod {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VerificationMethodVisitor;

        impl<'de> Visitor<'de> for VerificationMethodVisitor {
            type Value = VerificationMethod;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a verification method id string or object")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(VerificationMethod::Reference(Reference {
                    id: value.to_string(),
                    entity: None,
                }))
            }

            fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                Entity::deserialize(MapAccessDeserializer::new(map))
                    .map(VerificationMethod::Entity)
            }
        }

        deserializer.deserialize_any(VerificationMethodVisitor)
    }
}

impl fmt::Display for VerificationMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VerificationMethod{{id='{}'", self.id())?;
        if let (Some(method_type), Some(controller), Some(public_key)) = (
            self.method_type(),
            self.controller(),
            self.public_key_multibase(),
        ) {
            write!(
                f,
                ", type={}, controller={}, publicKeyMultibase='{}'",
                method_type,
                controller.to_did_string(),
                public_key
            )?;
        }
        f.write_str("}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "type",
        default, skip_serializing_if = "Option::is_none")]
    method_type: Option<VerificationMethodType>,

    #[serde(
        rename = "controller",
        default,
        serialize_with = "controller_serde::serialize",
        deserialize_with = "controller_serde::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    controller: Option<Id>,

    #[serde(
        rename = "publicKeyMultibase",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    public_key_multibase: Option<String>,
}

// Text formats use the `did:boson:<id>` form; other formats keep the raw id.
mod controller_serde {
    use super::*;

    pub(super) fn serialize<S>(value: &Option<Id>, se: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) if se.is_human_readable() => se.serialize_str(&v.to_did_string()),
            _ => codec::id_opt::serialize(value, se),
        }
    }

    pub(super) fn deserialize<'de, D>(de: D) -> std::result::Result<Option<Id>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !de.is_human_readable() {
            return codec::id_opt::deserialize(de);
        }

        Option::<String>::deserialize(de)?
            .map(|value| {
                if value.starts_with("did:") {
                    DIDUrl::parse(&value)
                        .map_err(serde::de::Error::custom)?
                        .id()
                        .copied()
                        .ok_or_else(|| serde::de::Error::custom("DID URL has no identifier"))
                } else {
                    Id::try_from(value.as_str()).map_err(serde::de::Error::custom)
                }
            })
            .transpose()
    }
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

#[derive(Debug, Clone, Eq)]
pub struct Reference {
    id: String,
    entity: Option<Entity>,
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
        self.entity
            .as_ref()
            .and_then(|v| v.public_key_multibase.as_deref())
    }

    pub(crate) fn is_reference(&self) -> bool {
        true
    }

    pub(crate) fn update_reference(&mut self, entity: Entity) -> Result<()> {
        if entity.is_reference() {
            return Err(ArgumentError::new(
                "Cannot update Reference with another Reference",
            ));
        }

        if entity.id != self.id {
            return Err(ArgumentError::new("Entity ID does not match Reference ID"));
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

#[cfg(test)]
#[path = "unitests/test_verification_method.rs"]
mod test_verification_method;
