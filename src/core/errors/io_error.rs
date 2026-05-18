use std::{
    io,
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct IOError {
    message: String
}

impl IOError {
    pub fn new(message: String) -> Box<Self> {
        Box::new(Self { message })
    }
}

impl Error for IOError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl From<io::Error> for Box<IOError> {
    fn from(err: io::Error) -> Box<IOError> {
        IOError::new(format!("IO error: {}", err))
    }
}

impl fmt::Display for IOError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IOError: {}", self.message)
     }
}
