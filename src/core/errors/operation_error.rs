use std::{error::Error, fmt};

#[derive(Debug)]
pub struct OperationError(String);

impl OperationError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for OperationError {}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
