use serde::{Serialize, Deserialize};
use super::method::RPCMethod;

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub(crate) struct RPCRequest<P, R>
where
    P: Serialize,
    R: Deserialize<'static>
{
    #[serde(rename = "i")]
    id: u32,

    #[serde(rename = "m")]
    method: RPCMethod,

    // This is the client side cookie for data sync between multiple device.
	// Because all messages go through the super node, so the sensitive data should
	// be encrypted(by user's key pair) can only can be decrypted by the user self-only.
	// The server should ignore this field.
    #[serde(rename = "p")]
    params: P,

    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    cookie: Option<Vec<u8>>,

    promise: Option<R>,
    response: Option<R>,
}

#[allow(dead_code)]
impl<P, R> RPCRequest<P, R>
where
    P: Serialize,
    R: Deserialize<'static>
{
    pub(crate) fn new(id: u32, method: RPCMethod, params: P) -> Self {
        Self {
            id,
            method,
            params,
            cookie: None,
            promise: None,
            response: None,
        }
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn method(&self) -> &RPCMethod {
        &self.method
    }

    pub(crate) fn params(&self) -> &P {
        &self.params
    }

    pub(crate) fn set_cookie(&mut self, cookie: Vec<u8>) {
        self.cookie = Some(cookie);
    }
}