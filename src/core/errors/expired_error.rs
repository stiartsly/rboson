use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct ExpiredError {
    message: String
}

impl ExpiredError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self { message: message.into() })
    }
}

impl Error for ExpiredError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for ExpiredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExpiredError: {}", self.message)
     }
}
