use crate::director::errors::DirectorError;
use std::{error::Error, fmt};
#[derive(Debug)]
pub struct ForbiddenError {
    status: u16,
    message: String,
}
impl ForbiddenError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 403,
            message: message.into(),
        })
    }
    pub fn status(&self) -> u16 {
        self.status
    }
}
impl fmt::Display for ForbiddenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director access forbidden: {}", self.message)
    }
}

impl Error for ForbiddenError {}

impl DirectorError for ForbiddenError {
    fn status(&self) -> u16 {
        self.status()
    }
}
