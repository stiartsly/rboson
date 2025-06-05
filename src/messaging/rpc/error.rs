use std::fmt;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

pub(crate) static SUPERNODE_ERR: Lazy<RPCError>  = Lazy::new(|| RPCError::new(-1, "Super node internal error", None));
pub(crate) static INVALID_PARAMS: Lazy<RPCError> = Lazy::new(|| RPCError::new(-2, "Invalid parameters", None));
pub(crate) static INVALID_METHOD: Lazy<RPCError> = Lazy::new(|| RPCError::new(-3, "Invalid method", None));
pub(crate) static FORBIDDEN: Lazy<RPCError>      = Lazy::new(|| RPCError::new(-4, "Forbidden", None));
pub(crate) static TIMEOUT: Lazy<RPCError>        = Lazy::new(|| RPCError::new(-5, "Timeout", None));
pub(crate) static NOT_UP_TO_DATE: Lazy<RPCError> = Lazy::new(|| RPCError::new(-6, "Not up to date", None));
pub(crate) static ALREADY_EXISTS: Lazy<RPCError> = Lazy::new(|| RPCError::new(-7, "Already exists", None));

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RPCError {
    #[serde(rename = "c")]
    code: i32,

    #[serde(rename = "m")]
    message: String,

    #[serde(rename = "d", skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

#[allow(dead_code)]
impl RPCError {
    pub fn new(code: i32, message: &str, data: Option<String>) -> Self {
        Self {
            code,
            message: message.to_string(),
            data,
        }
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn data(&self) -> Option<&String> {
        self.data.as_ref()
    }
}

impl fmt::Display for RPCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RPCError [code={}, message={}]", self.code, self.message)
    }
}
