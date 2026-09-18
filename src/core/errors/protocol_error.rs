use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ProtocolError(String);

impl ProtocolError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for ProtocolError {}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
