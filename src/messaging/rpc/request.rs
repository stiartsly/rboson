use serde::{Serialize, Deserialize};
use serde_cbor::{self};

use crate::{
    Id,
    Error,
    error::Result
};
use super::{
    method::RPCMethod,
    response::RPCResponse,
    promise::Promise,
    params::Parameters
};

#[derive(Serialize, Deserialize)]
pub(crate) struct RPCRequest
{
    #[serde(rename = "i")]
    id: u32,

    #[serde(rename = "m")]
    method: RPCMethod,

    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    params: Option<Parameters>,

    // This is the client side cookie for data sync between multiple device.
	// Because all messages go through the super node, so the sensitive data should
	// be encrypted(by user's key pair) can only can be decrypted by the user self-only.
	// The server should ignore this field.
    #[serde(rename = "c", skip_serializing_if = "crate::is_none_or_empty")]
    cookie: Option<Vec<u8>>,

    #[serde(skip)]
    promise: Option<Promise>,

    #[serde(skip)]
    _response: Option<RPCResponse>,

    #[serde(skip)]
    to: Option<Id>
}

impl RPCRequest
{
    pub(crate) fn new(id: u32, method: RPCMethod) -> Self {
        Self {
            id,
            method,
            params: None,
            cookie: None,
            promise: None,
            _response: None,
            to: None,
        }
    }

    pub(crate) fn recipient(&self) -> &Id {
        self.to.as_ref().unwrap()
    }

    pub(crate) fn from(body: &[u8]) -> Result<Self> {
        serde_cbor::from_slice::<RPCRequest>(body).map_err(|e|
            Error::Protocol(format!("Failed to parse RPC response: {}", e))
        )
    }

    pub(crate) fn with_recipient(mut self, recipient: Id) -> Self {
        self.to = Some(recipient);
        self
    }

    pub(crate) fn with_params(mut self, params: Parameters) -> Self {
        self.params = Some(params);
        self
    }

    pub(crate) fn with_promise(mut self, promise: Promise) -> Self{
        self.promise = Some(promise);
        self
    }

    pub(crate) fn with_cookie(mut self, cookie: Vec<u8>) -> Self {
        self.cookie = Some(cookie);
        self
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn params(&self) -> Option<&Parameters> {
        self.params.as_ref()
    }

    pub(crate) fn method(&self) -> RPCMethod {
        self.method
    }

    pub(crate) fn cookie(&self) -> Option<&[u8]> {
        self.cookie.as_deref()
    }

    #[allow(unused)]
    pub(crate) fn is_initiator(&self) -> bool {
        self.promise.is_some()
    }

    pub(crate) fn promise(&self) -> Option<&Promise> {
        self.promise.as_ref()
    }
}
