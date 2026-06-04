use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct PermissionError {
    message: String
}

impl PermissionError {
    pub fn new(message: impl Into<String>) -> Box<Self>  {
        Box::new(Self { message: message.into() })
    }
}

impl Error for PermissionError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for PermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PermissionError: {}", self.message)
    }
}

