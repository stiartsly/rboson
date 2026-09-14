use crate::director::errors::DirectorError;
use std::{error::Error, fmt};
#[derive(Debug)]
pub struct PassphraseRequiredError {
    status: u16,
    message: String,
}

impl PassphraseRequiredError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 428,
            message: message.into(),
        })
    }

    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for PassphraseRequiredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director passphrase required: {}", self.message)
    }
}

impl Error for PassphraseRequiredError {}

impl DirectorError for PassphraseRequiredError {
    fn status(&self) -> u16 {
        self.status()
    }
}
