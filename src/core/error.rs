use std::fmt;
use std::io;
use std::net;
use std::result;

#[derive(Debug)]
pub enum Error {
    NotImplemented(String),
    Argument(String),
    Io(String),
    Network(String),
    State(String),
    Protocol(String),
    Crypto(String),
    Db(String),
    Permission(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotImplemented(msg) => write!(f, "{}", msg),
            Error::Argument(msg)    => write!(f, "{}", msg),
            Error::State(msg)       => write!(f, "{}", msg),
            Error::Io(msg)          => write!(f, "{}", msg),
            Error::Network(msg)     => write!(f, "{}", msg),
            Error::Protocol(msg)    => write!(f, "{}", msg),
            Error::Crypto(msg)      => write!(f, "{}", msg),
            Error::Db(msg)          => write!(f, "{}", msg),
            Error::Permission(msg)  => write!(f, "{}", msg),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(format!("IO error: {}", err))
    }
}

impl From<net::AddrParseError> for Error {
    fn from(err: net::AddrParseError) -> Self {
        Error::Network(format!("Network error: {}", err))
    }
}

impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        Error::Db(format!("SQlite excutation error: {}", err))
    }
}

impl From<diesel::ConnectionError> for Error {
    fn from(err: diesel::ConnectionError) -> Self {
        Error::Db(format!("SQLite connection error: {}", err))
    }
}

pub type Result<T> = result::Result<T, Error>;
