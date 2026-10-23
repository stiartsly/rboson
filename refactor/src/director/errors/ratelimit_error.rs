use crate::director::errors::DirectorError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct RateLimitError {
    status: u16,
    message: String,
}

impl RateLimitError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 429,
            message: message.into(),
        })
    }

    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director rate limit exceeded: {}", self.message)
    }
}

impl Error for RateLimitError {}

impl DirectorError for RateLimitError {
    fn status(&self) -> u16 {
        self.status()
    }
}
