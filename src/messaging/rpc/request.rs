use serde::Serialize;
use super::{
    method::RPCMethod,
    response::RPCResponse,
    promise::Promise,
    parameters::Parameters
};

#[allow(unused)]
#[derive(Serialize)]
pub(crate) struct RPCRequest
{
    #[serde(rename = "i")]
    id: i32,

    #[serde(rename = "m")]
    method: RPCMethod,

    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    params: Option<Parameters>,

    // This is the client side cookie for data sync between multiple device.
	// Because all messages go through the super node, so the sensitive data should
	// be encrypted(by user's key pair) can only can be decrypted by the user self-only.
	// The server should ignore this field.
    //#[serde(rename = "c", with = "crate::serde_option_bytes_as_cbor")]
    #[serde(rename = "c", skip_serializing_if = "crate::is_none_or_empty")]
    cookie: Option<Vec<u8>>,

    #[serde(skip)]
    promise: Option<Promise>,

    #[serde(skip)]
    response: Option<RPCResponse>
}

#[allow(unused)]
impl RPCRequest
{
    pub(crate) fn new(id: i32, method: RPCMethod, params: Option<Parameters>) -> Self {
        Self {
            id,
            method,
            params,
            cookie: None,
            promise: None,
            response: None,
        }
    }

    pub(crate) fn with_promise(mut self, promise: Promise) -> Self{
        self.promise = Some(promise);
        self
    }

    pub(crate) fn with_cookie(mut self, cookie: Vec<u8>) -> Self {
        self.cookie = Some(cookie);
        self
    }

    pub(crate) fn id(&self) -> i32 {
        self.id
    }

    pub(crate) fn method(&self) -> RPCMethod {
        self.method
    }

    pub(crate) fn cookie(&self) -> Option<&[u8]> {
        self.cookie.as_deref()
    }

    pub(crate) fn complete<R>(&mut self, response: RPCResponse)
    where
        R: serde::de::DeserializeOwned,
    {
        self.response = Some(response);

        // TODO:
    }
}