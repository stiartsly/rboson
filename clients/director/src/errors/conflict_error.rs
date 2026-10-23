use crate::director::errors::DirectorError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ConflictError {
    status: u16,
    message: String,
}

impl ConflictError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 409,
            message: message.into(),
        })
    }

    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for ConflictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Director request conflicts with existing state: {}",
            self.message
        )
    }
}

impl Error for ConflictError {}

impl DirectorError for ConflictError {
    fn status(&self) -> u16 {
        self.status()
    }
}
