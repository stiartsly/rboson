use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct CryptoError {
    message: String
}

impl CryptoError {
    pub fn new(message: String) -> Box<Self>  {
        Box::new(Self { message })
    }
}

impl Error for CryptoError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CryptoError: {}", self.message)
    }
}
