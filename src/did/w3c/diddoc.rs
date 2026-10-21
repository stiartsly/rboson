use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use crate::{
    errors::{ArgumentError, Result, SignatureError},
    utils::codec, CryptoIdentity, Id,
};

use crate::did::{
    card::{Card, Service as CardService},
    did_constants as constants,
    proof::{Proof, ProofPurpose, ProofType},
    w3c::{DIDDocumentBuilder, VerifiableCredential as VC},
    DIDUrl, VerificationMethod as VM, VerificationMethodType,
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct DIDDocument {
    #[serde(
        rename = "@context",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    contexts: Option<Vec<String>>,

    #[serde(
        rename = "id",
        serialize_with = "codec::id::serialize",
        deserialize_with = "codec::id::deserialize"
    )]
    id: Id,

    #[serde(
        rename = "verificationMethod",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    verification_methods: Option<Vec<VM>>,

    #[serde(
        rename = "authentication",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    authentications: Option<Vec<VM>>,

    #[serde(
        rename = "assertion",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    assertions: Option<Vec<VM>>,

    #[serde(
        rename = "verifiableCredential",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    credentials: Option<Vec<VC>>,

    #[serde(
        rename = "service",
        default,
        skip_serializing_if = "codec::is_default"
    )]
    services: Option<Vec<Service>>,

    #[serde(
        rename = "proof",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    proof: Option<Proof>,
}

impl DIDDocument {
    pub(crate) fn unsigned(
        contexts: Vec<String>,
        id: Id,
        vms: Vec<VM>,
        auths: Vec<VM>,
        assertions: Vec<VM>,
        credentials: Vec<VC>,
        services: Vec<Service>,
    ) -> Self {
        let contexts = match !contexts.is_empty() {
            true => Some(contexts),
            false => None,
        };
        let verification_methods = match !vms.is_empty() {
            true => Some(vms),
            false => None,
        };
        let authentications = match !auths.is_empty() {
            true => Some(auths),
            false => None,
        };
        let assertions = match !assertions.is_empty() {
            true => Some(assertions),
            false => None,
        };
        let credentials = match !credentials.is_empty() {
            true => Some(credentials),
            false => None,
        };
        let services = match !services.is_empty() {
            true => Some(services),
            false => None,
        };

        Self {
            contexts,
            id,
            verification_methods,
            authentications,
            assertions,
            credentials,
            services,
            proof: None,
        }
    }

    pub(crate) fn signed(mut unsigned: Self, proof: Option<Proof>) -> Self {
        unsigned.proof = proof;
        unsigned
    }

    pub fn from_card(card: &Card) -> Self {
        Self::from_card_with_contexts(card, Vec::new(), HashMap::new())
    }

    pub fn from_card_with_contexts(
        card: &Card,
        doc_contexts: Vec<&str>,
        vctype_contexts: HashMap<&str, Vec<&str>>,
    ) -> Self {
        if let Some(doc) = card.did_doc() {
            return doc.clone();
        }

        let mut contexts = vec![
            constants::W3C_DID_CONTEXT,
            constants::BOSON_DID_CONTEXT,
            constants::W3C_ED25519_CONTEXT,
        ];

        for context in doc_contexts {
            if !contexts.contains(&context) {
                contexts.push(context);
            }
        }

        let default_method = VM::default_entity(card.id());
        let default_method_ref = default_method.to_reference();

        let unsigned = Self::unsigned(
            contexts.iter().map(|s| s.to_string()).collect(),
            card.id().clone(),
            vec![default_method],
            vec![default_method_ref.clone()],
            vec![default_method_ref.clone()],
            card.credentials()
                .iter()
                .map(|c| VC::from_cred_with_type_contexts(c, Some(vctype_contexts.clone())))
                .collect(),
            card.services()
                .iter()
                .map(|s| {
                    Service::new(
                        s.id().to_string(),
                        s.service_type().to_string(),
                        s.endpoint().to_string(),
                        s.properties_map().clone(),
                    )
                })
                .collect(),
        );
        let proof = Proof::new(
            ProofType::Ed25519Signature2020,
            card.signed_at().unwrap_or(SystemTime::now()),
            default_method_ref,
            ProofPurpose::AssertionMethod,
            card.signature().to_vec(),
        );
        Self::signed(unsigned, Some(proof))
    }

    pub fn contexts(&self) -> Vec<&str> {
        self.contexts
            .as_ref()
            .map_or(Vec::new(), |v| v.iter().map(|s| s.as_str()).collect())
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn verification_methods(&self) -> Vec<&VM> {
        self.verification_methods
            .as_ref()
            .map_or(Vec::new(), |v| v.iter().collect())
    }

    pub fn verification_methods_by_type(&self, method_type: VerificationMethodType) -> Vec<&VM> {
        self.verification_methods.as_ref().map_or(Vec::new(), |vs| {
            vs.iter()
                .filter(|v| v.method_type() == Some(method_type))
                .collect()
        })
    }

    pub fn verification_method(&self, id: &str) -> Option<&VM> {
        let didurl = match id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            true => DIDUrl::parse(id).ok()?,
            false => DIDUrl::new(&self.id, None, None, Some(id)),
        };
        self.verification_method_by_didurl(&didurl)
    }

    pub fn verification_method_by_didurl(&self, id: &DIDUrl) -> Option<&VM> {
        let id_str = id.to_string();
        self.verification_methods
            .as_ref()
            .and_then(|vs| vs.iter().find(|v| v.id() == id_str))
    }

    pub fn authentications(&self) -> Vec<&VM> {
        self.authentications
            .as_ref()
            .map_or(Vec::new(), |v| v.iter().collect())
    }

    pub fn authentication(&self, id: &str) -> Option<&VM> {
        let didurl = match id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            true => DIDUrl::parse(id).ok()?,
            false => DIDUrl::new(&self.id, None, None, Some(id)),
        };
        self.authentication_by_didurl(&didurl)
    }

    pub fn authentication_by_didurl(&self, id: &DIDUrl) -> Option<&VM> {
        let id_str = id.to_string();
        self.authentications
            .as_ref()
            .map(|v| v.iter().find(|v| v.id() == id_str))
            .flatten()
    }

    pub fn assertions(&self) -> Vec<&VM> {
        self.assertions
            .as_ref()
            .map_or(Vec::new(), |v| v.iter().collect())
    }

    pub fn assertion(&self, id: &str) -> Option<&VM> {
        let didurl = match id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            true => DIDUrl::parse(id).ok()?,
            false => DIDUrl::new(&self.id, None, None, Some(id)),
        };
        self.assertion_by_didurl(&didurl)
    }

    pub fn assertion_by_didurl(&self, id: &DIDUrl) -> Option<&VM> {
        let id_str = id.to_string();
        self.assertions
            .as_ref()
            .map(|v| v.iter().find(|v| v.id() == id_str))
            .flatten()
    }

    pub fn credentials(&self) -> Vec<&VC> {
        self.credentials
            .as_ref()
            .map_or(Vec::new(), |v| v.iter().collect())
    }

    pub fn credentials_by_type(&self, credential_type: &str) -> Vec<&VC> {
        self.credentials.as_ref().map_or(Vec::new(), |v| {
            v.iter()
                .filter(|vc| vc.types().contains(&credential_type))
                .collect()
        })
    }

    pub fn credential(&self, id: &str) -> Option<&VC> {
        let didurl = match id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            true => DIDUrl::parse(id).ok()?,
            false => DIDUrl::new(&self.id, None, None, Some(id)),
        };
        self.credential_by_didurl(&didurl)
    }

    pub fn credential_by_didurl(&self, id: &DIDUrl) -> Option<&VC> {
        let id_str = id.to_string();
        self.credentials
            .as_ref()
            .and_then(|values| values.iter().find(|vc| vc.id() == id_str))
    }

    pub fn credential_with_type(&self, id: &str, credential_type: &str) -> Option<&VC> {
        let did_url = if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            DIDUrl::parse(id).ok()?
        } else {
            DIDUrl::new(&self.id, None, None, Some(id))
        };
        self.credential_by_didurl_with_type(&did_url, credential_type)
    }

    pub fn credential_by_didurl_with_type(
        &self,
        id: &DIDUrl,
        credential_type: &str,
    ) -> Option<&VC> {
        let id = id.to_string();
        self.credentials.as_ref().and_then(|values| {
            values.iter().find(|credential| {
                credential.id() == id
                    && credential.types().contains(&credential_type)
            })
        })
    }

    pub fn services(&self) -> Vec<&Service> {
        self.services
            .as_ref()
            .map_or(Vec::new(), |v| v.iter().collect())
    }

    pub fn services_by_type(&self, service_type: &str) -> Vec<&Service> {
        self.services.as_ref().map_or(Vec::new(), |v| {
            v.iter()
                .filter(|s| s.service_type() == service_type)
                .collect()
        })
    }

    pub fn service(&self, id: &str) -> Option<&Service> {
        let didurl = match id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            true => DIDUrl::parse(id).ok()?,
            false => DIDUrl::new(&self.id, None, None, Some(id)),
        };
        self.service_by_didurl(&didurl)
    }

    pub fn service_by_didurl(&self, id: &DIDUrl) -> Option<&Service> {
        let id_str = id.to_string();
        self.services
            .as_ref()
            .and_then(|values| values.iter().find(|service| service.id() == id_str))
    }

    pub fn service_with_type(&self, id: &str, service_type: &str) -> Option<&Service> {
        let did_url = if id.starts_with(constants::DID_SUFFIXED_SCHEME) {
            DIDUrl::parse(id).ok()?
        } else {
            DIDUrl::new(&self.id, None, None, Some(id))
        };
        self.service_by_didurl_with_type(&did_url, service_type)
    }

    pub fn service_by_didurl_with_type(
        &self,
        id: &DIDUrl,
        service_type: &str,
    ) -> Option<&Service> {
        let id = id.to_string();
        self.services.as_ref().and_then(|values| {
            values.iter().find(|service| {
                service.id() == id && service.service_type() == service_type
            })
        })
    }

    pub fn proof(&self) -> &Proof {
        self.proof.as_ref().unwrap()
    }

    pub fn is_genuine(&self) -> bool {
        self.proof
            .as_ref()
            .map(|v| v.verify(&self.id, &self.to_sign_data()))
            .unwrap_or(false)
    }

    pub fn validate(&self) -> Result<()> {
        match self.is_genuine() {
            true => Ok(()),
            false => Err(SignatureError::new("Document signature is not valid")),
        }
    }

    /// Validates the structural constraints Java's constructor enforces.
    pub fn validate_structure(&self) -> Result<()> {
        let methods = self.verification_methods();
        if methods.iter().any(|method| method.is_reference()) {
            return Err(ArgumentError::new(
                "verificationMethod must not contain references",
            )
            .into());
        }
        for method in self.authentications() {
            if method.is_reference()
                && !methods.iter().any(|candidate| candidate.id() == method.id())
            {
                return Err(ArgumentError::new(
                    "authentication contains an invalid reference",
                )
                .into());
            }
        }
        for method in self.assertions() {
            if method.is_reference()
                && !methods.iter().any(|candidate| candidate.id() == method.id())
            {
                return Err(ArgumentError::new(
                    "assertion contains an invalid reference",
                )
                .into());
            }
        }
        Ok(())
    }

    pub(crate) fn to_sign_data(&self) -> Vec<u8> {
        Card::signed(
            self.to_unsigned_boson_card(),
            self.proof.as_ref().map(|p| p.created()),
            None,
        )
        .to_sign_data()
    }

    pub(crate) fn to_unsigned_boson_card(&self) -> Card {
        let creds = self.credentials.as_ref().map_or(Vec::new(), |v| {
            v.iter().map(|c| c.to_boson_credential()).collect()
        });
        let services = self.services.as_ref().map_or(Vec::new(), |v| {
            v.iter()
                .map(|s| {
                    CardService::new(
                        s.id.clone(),
                        s.service_type.clone(),
                        s.service_endpoint.clone(),
                        s.properties.clone(),
                    )
                })
                .collect()
        });

        Card::unsigned(
            self.id.clone(),
            (!creds.is_empty()).then_some(creds),
            (!services.is_empty()).then_some(services),
            Some(self.clone()),
        )
    }

    pub fn to_card(&self) -> Card {
        self.to_boson_card()
    }

    pub fn to_boson_card(&self) -> Card {
        Card::signed(
            self.to_unsigned_boson_card(),
            self.proof.as_ref().map(|p| p.created()),
            self.proof.as_ref().map(|p| p.proof_value().to_vec()),
        )
    }

    pub fn parse(json: &str) -> Result<Self> {
        Self::try_from(json)
    }

    pub fn parse_bytes(cbor: &[u8]) -> Result<Self> {
        Self::try_from(cbor)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_cbor::to_vec(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode DIDDocument: {error}")).into()
        })
    }

    pub fn to_pretty_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|error| {
            ArgumentError::new(format!("Failed to encode DIDDocument: {error}")).into()
        })
    }

    pub fn builder(subject: CryptoIdentity) -> DIDDocumentBuilder {
        DIDDocumentBuilder::new(subject)
    }
}

impl PartialEq<Self> for DIDDocument {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.contexts == other.contexts
            && self.verification_methods == other.verification_methods
            && self.authentications == other.authentications
            && self.assertions == other.assertions
            && self.credentials == other.credentials
            && self.services == other.services
            && self.proof == other.proof
    }
}

impl TryFrom<&str> for DIDDocument {
    type Error = Box<dyn Error>;

    fn try_from(data: &str) -> Result<Self> {
        let document: Self = serde_json::from_str(data).map_err(|e| -> crate::Error {
            ArgumentError::new(format!("Failed to parse DIDDocument from string: {}", e)).into()
        })?;
        document.validate_structure()?;
        Ok(document)
    }
}

impl FromStr for DIDDocument {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s)
    }
}

impl TryFrom<&[u8]> for DIDDocument {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self> {
        let document: Self = serde_cbor::from_slice(data).map_err(|e| -> crate::Error {
            ArgumentError::new(format!("Failed to parse DIDDocument from bytes: {}", e)).into()
        })?;
        document.validate_structure()?;
        Ok(document)
    }
}

impl From<&DIDDocument> for Vec<u8> {
    fn from(doc: &DIDDocument) -> Self {
        serde_cbor::to_vec(doc).unwrap()
    }
}

impl From<&Card> for DIDDocument {
    fn from(card: &Card) -> Self {
        Self::from_card(card)
    }
}

impl From<&DIDDocument> for Card {
    fn from(doc: &DIDDocument) -> Self {
        doc.to_boson_card()
    }
}

impl fmt::Display for DIDDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self).map_err(|_| fmt::Error)?.fmt(f)
    }
}

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Service {
    #[serde(rename = "id")]
    id: String,

    #[serde(rename = "type")]
    service_type: String,

    #[serde(rename = "serviceEndpoint")]
    service_endpoint: String,

    properties: Map<String, Value>,
}

impl Service {
    pub(crate) fn new(
        id: String,
        service_type: String,
        endpoint: String,
        properties: Map<String, Value>,
    ) -> Self {
        Self {
            id,
            service_type,
            service_endpoint: endpoint,
            properties,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn service_type(&self) -> &str {
        &self.service_type
    }

    pub fn service_endpoint(&self) -> &str {
        &self.service_endpoint
    }

    pub fn properties(&self) -> &Map<String, Value> {
        &self.properties
    }
}

impl PartialEq<Self> for Service {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.service_type == other.service_type
            && self.service_endpoint == other.service_endpoint
            && self.properties == other.properties
    }
}
