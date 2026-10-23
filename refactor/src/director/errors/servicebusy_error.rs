use crate::director::errors::DirectorError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ServiceBusyError {
    status: u16,
    message: String,
}

impl ServiceBusyError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            status: 503,
            message: message.into(),
        })
    }
    pub fn status(&self) -> u16 {
        self.status
    }
}

impl fmt::Display for ServiceBusyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Director service is busy: {}", self.message)
    }
}

impl Error for ServiceBusyError {}

impl DirectorError for ServiceBusyError {
    fn status(&self) -> u16 {
        self.status()
    }
}
