use std::{error::Error, fmt, io};

#[derive(Debug)]
pub struct IOError(String);

impl IOError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for IOError {}

impl From<io::Error> for Box<IOError> {
    fn from(err: io::Error) -> Box<IOError> {
        IOError::new(format!("{}", err))
    }
}

impl fmt::Display for IOError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
