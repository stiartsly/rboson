use crate::director::errors::DirectorError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct UnauthorizedError {
    status: u16,
    message: String,
}
impl UnauthorizedError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 401,
            message: message.into(),
        })
    }

    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for UnauthorizedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director authentication failed: {}", self.message)
    }
}

impl Error for UnauthorizedError {}

impl DirectorError for UnauthorizedError {
    fn status(&self) -> u16 {
        self.status()
    }
}
