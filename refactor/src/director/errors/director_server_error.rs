use crate::director::errors::DirectorError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ServerError {
    status: u16,
    message: String,
}
impl ServerError {
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
impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director server error: {}", self.message)
    }
}

impl Error for ServerError {}

impl DirectorError for ServerError {
    fn status(&self) -> u16 {
        self.status()
    }
}
