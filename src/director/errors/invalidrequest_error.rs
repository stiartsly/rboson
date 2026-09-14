use crate::director::errors::DirectorError;
use std::{error::Error, fmt};
#[derive(Debug)]
pub struct InvalidRequestError {
    status: u16,
    message: String,
}

impl InvalidRequestError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 400,
            message: message.into(),
        })
    }
    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for InvalidRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid Director request: {}", self.message)
    }
}

impl Error for InvalidRequestError {}

impl DirectorError for InvalidRequestError {
    fn status(&self) -> u16 {
        self.status()
    }
}
