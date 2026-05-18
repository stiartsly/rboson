use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct ProtocolError {
    message: String
}

impl ProtocolError {
    pub fn new(message: String) -> Box<Self> {
        Box::new(Self { message })
    }
}

impl Error for ProtocolError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProtocolError: {}", self.message)
    }
}
