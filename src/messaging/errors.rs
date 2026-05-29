use std::fmt;

/// Errors that can occur in the messaging subsystem.
#[derive(Debug)]
pub enum Error {
    /// Generic I/O or network error.
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
    Timeout,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e)                        => write!(f, "IO error: {}", e),
            Error::Argument(m)                  => write!(f, "Invalid argument: {}", m),
            Error::Protocol { code, message }   => write!(f, "Protocol error {}: {}", code, message),
            Error::State(m)                     => write!(f, "State error: {}", m),
            Error::Encoding(m)                  => write!(f, "Encoding error: {}", m),
            Error::Auth(m)                      => write!(f, "Auth error: {}", m),
            Error::NotFound(m)                  => write!(f, "Not found: {}", m),
            Error::Timeout                      => write!(f, "Operation timed out"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
