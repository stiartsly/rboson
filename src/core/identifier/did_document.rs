use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    Id,
	error::{Error, Result},
	core::crypto_identity::CryptoIdentity,
};

use crate::core::identifier::{
	did_constants::DID_SCHEME as did_scheme,
    VerificationMethod,
	VerificationMethodType,
	VerifiableCredential,
    Proof,
	DIDUrl,
	card::Card,
	DIDDocumentBuilder,
};

#[derive(Debug, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct DIDDocument {
    #[serde(rename = "@context", skip_serializing_if = "Vec::is_empty")]
    contexts: Vec<String>,

    #[serde(rename = "id")]
    id: Id,

    #[serde(rename = "verificationMethod", skip_serializing_if = "Vec::is_empty")]
    verification_methods: Vec<VerificationMethod>,

    #[serde(rename = "authentication", skip_serializing_if = "Vec::is_empty")]
    authentications: Vec<VerificationMethod>,

    #[serde(rename = "assertion", skip_serializing_if = "Vec::is_empty")]
    assertions: Vec<VerificationMethod>,

    #[serde(rename = "verifiableCredential", skip_serializing_if = "Vec::is_empty")]
    credentials: Vec<VerifiableCredential>,

    #[serde(rename = "service", skip_serializing_if = "Vec::is_empty")]
    services: Vec<Service>, // Assuming Service is a String for simplicity

    #[serde(rename = "proof")]
    proof: Option<Proof>,
}

#[allow(unused)]
impl DIDDocument {
	pub(crate) fn unsigned(
		contexts: Vec<String>,
		id: Id,
		verification_methods: Vec<VerificationMethod>,
		authentications: Vec<VerificationMethod>,
		assertions: Vec<VerificationMethod>,
		credentials: Vec<VerifiableCredential>,
		services: Vec<Service>
	) -> Self {
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

	pub(crate) fn signed(
		mut unsigned: Self,
		proof: Option<Proof>
	) -> Self {
		unsigned.proof = proof;
		unsigned
	}

	pub fn from_card(card: &Card) -> Self {
		Self::from_card_with_contexts(card, None, None)
	}

	pub fn from_card_with_contexts(
		_card: &Card,
		_doc_contexts: Option<Vec<String>>,
		_vctype_contexts: Option<HashMap<String, Vec<String>>>
	) -> Self {
		unimplemented!()
	}

	pub fn contexts(&self) -> Vec<&str> {
		self.contexts.iter().map(|v| v.as_str()).collect()
	}

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn verification_methods(&self) -> Vec<&VerificationMethod> {
		self.verification_methods.iter().collect()
    }

    pub fn verification_methods_by_type(
		&self,
		method_type: VerificationMethodType
	) -> Vec<&VerificationMethod> {
        self.verification_methods
            .iter()
            .filter(|v| v.method_type().map(|v| v== method_type).unwrap_or(false))
			.collect()
    }

	pub fn verification_method_by_id(
		&self,
		id: &str
	) -> Option<&VerificationMethod> {
		let didurl = match id.starts_with(did_scheme) {
			true => DIDUrl::parse(id).unwrap(),
			false => DIDUrl::new(&self.id, None, None, Some(id))
		};
		self.verification_method_by_didurl(&didurl)
	}

	pub fn verification_method_by_didurl(
		&self,
		id: &DIDUrl
	) -> Option<&VerificationMethod> {
		let id_str = id.to_string();
		self.verification_methods.iter()
			.find(|v| v.id() == id_str)
	}

	pub fn authentications(&self) -> Vec<&VerificationMethod> {
		self.authentications.iter().collect()
	}

	pub fn authentication_by_id(
		&self,
		id: &str
	) -> Option<&VerificationMethod> {
		let didurl = match id.starts_with(did_scheme) {
			true => DIDUrl::parse(id).unwrap(),
			false => DIDUrl::new(&self.id, None, None, Some(id))
		};
		self.authentication_by_url(&didurl)
	}

	pub fn authentication_by_url(
		&self,
		id: &DIDUrl
	) -> Option<&VerificationMethod> {
		let id_str = id.to_string();
		self.authentications.iter()
			.find(|v| v.id() == id_str)
	}

	pub fn assertions(&self) -> Vec<&VerificationMethod> {
		self.assertions.iter().collect()
	}

	pub fn assertion_by_id(
		&self,
		id: &str
	) -> Option<&VerificationMethod> {
		let didurl = match id.starts_with(did_scheme) {
			true => DIDUrl::parse(id).unwrap(),
			false => DIDUrl::new(&self.id, None, None, Some(id))
		};
		self.assertion_by_url(&didurl)
	}

	pub fn assertion_by_url(
		&self,
		id: &DIDUrl
	) -> Option<&VerificationMethod> {
		let id_str = id.to_string();
		self.assertions.iter()
			.find(|v| v.id() == id_str)
	}

	pub fn credentials(&self) -> Vec<&VerifiableCredential> {
		self.credentials.iter().collect()
	}

	pub fn credentials_by_type(
		&self,
		credential_type: &str
	) -> Vec<&VerifiableCredential> {
		self.credentials.iter()
			.filter(|vc| vc.types().contains(&credential_type))
			.collect()
	}

	pub fn credential_by_id(
		&self,
		id: &str
	) -> Option<&VerifiableCredential> {
		let didurl = match id.starts_with(did_scheme) {
			true => DIDUrl::parse(id).unwrap(),
			false => DIDUrl::new(&self.id, None, None, Some(id))
		};
		self.credential_by_didurl(&didurl)
	}

	pub fn credential_by_didurl(
		&self,
		id: &DIDUrl
	) -> Option<&VerifiableCredential> {
		let id_str = id.to_string();
		self.credentials.iter()
			.find(|vc| vc.id() == id_str)
	}

	pub fn services(&self) -> Vec<&Service> {
		self.services.iter().collect()
	}

	pub fn services_by_type(
		&self,
		service_type: &str
	) -> Vec<&Service> {
		self.services.iter()
			.filter(|s| s.service_type() == service_type)
			.collect()
	}

	pub fn service_by_id(
		&self,
		id: &str
	) -> Option<&Service> {
		let didurl = match id.starts_with(did_scheme) {
			true => DIDUrl::parse(id).unwrap(),
			false => DIDUrl::new(&self.id, None, None, Some(id))
		};
		self.service_by_didurl(&didurl)
	}

	pub fn service_by_didurl(
		&self,
		id: &DIDUrl
	) -> Option<&Service> {
		let id_str = id.to_string();
		self.services.iter()
			.find(|s| s.id() == id_str)
	}

	pub fn proof(&self) -> &Proof {
		self.proof.as_ref().unwrap()
	}

	pub fn is_geniune(&self) -> bool {
		self.proof.as_ref().map(|v| v.verify(
			&self.id,
			&self.to_sign_data()
		)).unwrap_or(false)
	}

	pub fn validate(&self) -> Result<()> {
		match self.is_geniune() {
            true => Ok(()),
            false => Err(Error::Signature("Document signature is not valid".into())),
        }
	}

	pub(crate) fn to_sign_data(&self) -> Vec<u8> {
		unimplemented!()
	}

	pub fn builder(&self, subject: CryptoIdentity) -> DIDDocumentBuilder {
		DIDDocumentBuilder::new(subject)
	}
}

impl PartialEq<Self> for DIDDocument {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id &&
		self.contexts == other.contexts &&
		self.verification_methods == other.verification_methods &&
		self.authentications == other.authentications &&
		self.assertions == other.assertions &&
		self.credentials == other.credentials &&
		self.services == other.services &&
		self.proof == other.proof
	}
}

impl TryFrom<&str> for DIDDocument {
    type Error = Error;

    fn try_from(data: &str) -> Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            Error::Argument(format!("Failed to parse DIDDocument from string: {}", e))
        })
    }
}

impl TryFrom<&[u8]> for DIDDocument {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|e| {
            Error::Argument(format!("Failed to parse DIDDocument from bytes: {}", e))
        })
    }
}

impl From<&DIDDocument> for String {
    fn from(doc: &DIDDocument) -> Self {
        serde_json::to_string(&doc).unwrap()
    }
}

impl From<&DIDDocument> for Vec<u8> {
    fn from(doc: &DIDDocument) -> Self {
        serde_json::to_vec(doc).unwrap()
    }
}

impl From<&Card> for DIDDocument {
	fn from(card: &Card) -> Self {
		Self::from_card(card)
	}
}

impl From<&DIDDocument> for Card {
	fn from(_doc: &DIDDocument) -> Self {
		unimplemented!()
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

#[allow(unused)]
impl Service {
	pub(crate) fn new(
		id: String,
		service_type: String,
		endpoint: String,
		properties: Map<String, Value>
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
		self.id == other.id &&
		self.service_type == other.service_type &&
		self.service_endpoint == other.service_endpoint &&
		self.properties == other.properties
	}
}
