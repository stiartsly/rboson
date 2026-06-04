use std::{
    fmt,
    net,
    error::Error
};

#[derive(Debug)]
pub struct NetworkError {
    message: String
}

impl NetworkError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self { message: message.into() })
    }
}

impl Error for NetworkError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl From<net::AddrParseError> for Box<NetworkError> {
    fn from(err: net::AddrParseError) -> Box<NetworkError> {
        NetworkError::new(format!("Network error: {}", err))
    }
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NetworkError: {}", self.message)
    }
}
