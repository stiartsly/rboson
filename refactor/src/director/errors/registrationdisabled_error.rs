use crate::director::errors::DirectorError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct RegistrationDisabledError {
    status: u16,
    message: String,
}

impl RegistrationDisabledError {
    pub fn new(status: u16, message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: status,
            message: message.into(),
        })
    }
    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for RegistrationDisabledError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director registration is disabled: {}", self.message)
    }
}

impl Error for RegistrationDisabledError {}

impl DirectorError for RegistrationDisabledError {
    fn status(&self) -> u16 {
        self.status()
    }
}
