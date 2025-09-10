use serde::{Serialize, Deserialize};
use super::{
    method::RPCMethod,
    response::Response,
};

#[allow(dead_code)]
#[derive(Serialize)]
pub(crate) struct RPCRequest<P, R>
where
    P: Serialize,
    R: Deserialize<'static>
{
    #[serde(rename = "i")]
    id: i32,

    #[serde(rename = "m")]
    method: RPCMethod,

    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    params: Option<P>,

    // This is the client side cookie for data sync between multiple device.
	// Because all messages go through the super node, so the sensitive data should
	// be encrypted(by user's key pair) can only can be decrypted by the user self-only.
	// The server should ignore this field.
    #[serde(rename = "c", skip_serializing_if = "crate::is_none_or_empty")]
    cookie: Option<Vec<u8>>,

    #[serde(skip)]
    promise: Option<Box<dyn Fn(R)>>,
    #[serde(skip)]
    response: Option<Response<R>>,
}

#[allow(dead_code)]
impl<P, R> RPCRequest<P, R>
where
    P: Serialize,
    R: Deserialize<'static>
{
    pub(crate) fn new(id: i32, method: RPCMethod, params: Option<P>) -> Self {
        Self {
            id,
            method,
            params,
            cookie: None,
            promise: None,
            response: None,
        }
    }

    pub(crate) fn id(&self) -> i32 {
        self.id
    }

    pub(crate) fn method(&self) -> &RPCMethod {
        &self.method
    }

    pub(crate) fn params(&self) -> &Option<P> {
        &self.params
    }

    pub(crate) fn set_cookie(&mut self, cookie: Vec<u8>) {
        self.cookie = Some(cookie);
    }

    pub(crate) fn apply_with_cookie<F, T>(&mut self, cookie: T,  transform: F)
    where
        F: Fn(T) -> Vec<u8>,
    {
        self.cookie = Some(transform(cookie));
    }

    pub(crate) fn cookie(&self) -> Option<&[u8]> {
        self.cookie.as_deref()
    }
}