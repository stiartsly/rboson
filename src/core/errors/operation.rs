use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct OperationError {
    message: String
}

impl OperationError {
    pub fn new(message: impl Into<String>) -> Box<Self>  {
        Box::new(Self { message: message.into() })
    }
}

impl Error for OperationError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unsupported OperationError: {}", self.message)
    }
}

