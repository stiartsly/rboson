use crate::director::errors::DirectorError;
use std::{error::Error, fmt};
#[derive(Debug)]
pub struct NotFoundError {
    status: u16,
    message: String,
}

impl NotFoundError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 404,
            message: message.into(),
        })
    }
    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director resource not found: {}", self.message)
    }
}

impl Error for NotFoundError {}

impl DirectorError for NotFoundError {
    fn status(&self) -> u16 {
        self.status()
    }
}
