use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct MalformedError {
    message: String
}

impl MalformedError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self { message: message.into() })
    }
}

impl Error for MalformedError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for MalformedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MalformedError: {}", self.message)
     }
}
