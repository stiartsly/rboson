use std::{error::Error, fmt, net};

#[derive(Debug)]
pub struct NetworkError(String);

impl NetworkError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for NetworkError {}

impl From<net::AddrParseError> for Box<NetworkError> {
    fn from(err: net::AddrParseError) -> Box<NetworkError> {
        NetworkError::new(format!("{}", err))
    }
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
