use std::{error::Error, fmt};

#[derive(Debug)]
pub struct DBError(String);

impl DBError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for DBError {}

impl From<diesel::result::Error> for Box<DBError> {
    fn from(err: diesel::result::Error) -> Box<DBError> {
        DBError::new(format!("{}", err))
    }
}

impl From<diesel::ConnectionError> for Box<DBError> {
    fn from(err: diesel::ConnectionError) -> Box<DBError> {
        DBError::new(format!("{}", err))
    }
}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
