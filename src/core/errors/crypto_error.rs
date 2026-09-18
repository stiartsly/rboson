use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CryptoError(String);

impl CryptoError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for CryptoError {}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
