use std::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Error {
    #[serde(rename = "c")]
    code: i32,
    #[serde(rename = "m")]
    description: String,
}

impl Error {
    pub(crate) fn new(code: i32, description: impl Into<String>) -> Self {
        Self { code, description: description.into() }
    }

    pub(crate) fn code(&self) -> i32 {
        self.code
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
