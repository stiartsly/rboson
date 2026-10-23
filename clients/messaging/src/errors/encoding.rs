use std::{error::Error, fmt};

#[derive(Debug)]
pub struct EncodingError(String);
impl EncodingError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for EncodingError {}

impl fmt::Display for EncodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/*
Io(std::io::Error),
    /// Invalid argument or parameter.
    Argument(String),
    /// Protocol-level error (server returned an error code).
    Protocol { code: i32, message: String },
    /// The client is in an invalid state for the requested operation.
    State(String),
    /// Serialization / deserialization failure.
    Encoding(String),
    /// Authentication or signature verification failed.
    Auth(String),
    /// The requested item was not found.
    NotFound(String),
    /// Operation timed out.
*/
