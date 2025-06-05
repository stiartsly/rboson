use serde::{Deserialize, Serialize};

use super::error::RPCError;

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Response<R>{
    #[serde(rename = "i")]
    id: u32,

    #[serde(rename = "r", skip_serializing_if = "Option::is_none")]
    result: Option<R>,

    #[serde(rename = "e", skip_serializing_if = "Option::is_none")]
    error: Option<RPCError>,
}

#[allow(dead_code)]
impl<R> Response<R> {
    pub(crate) fn with_result(id: u32, result: R) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub(crate) fn with_error(id: u32, error: RPCError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
        }
    }

    pub(crate) fn with_error_details(id: u32, code: i32, message: &str, data: Option<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(RPCError::new(code, message, data)),
        }
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    pub(crate) fn failed(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn result(&self) -> Option<&R> {
        self.result.as_ref()
    }

    pub(crate) fn error(&self) -> Option<&RPCError> {
        self.error.as_ref()
    }

    // TODO:
}
