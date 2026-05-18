use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct SignatureError {
    message: String
}

impl SignatureError {
    pub fn new(message: String) -> Box<Self> {
        Box::new(Self { message })
    }
}

impl Error for SignatureError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for SignatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SignatureError: {}", self.message)
     }
}
