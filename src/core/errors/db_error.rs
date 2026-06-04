use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct DBError {
    message: String
}

impl DBError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self { message: message.into() })
    }
}

impl Error for DBError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl From<diesel::result::Error> for Box<DBError> {
    fn from(err: diesel::result::Error) -> Box<DBError> {
        DBError::new(format!("SQlite excutation error: {}", err))
    }
}

impl From<diesel::ConnectionError> for Box<DBError> {
    fn from(err: diesel::ConnectionError) -> Box<DBError> {
        DBError::new(format!("SQLite connection error: {}", err))
    }
}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DBError: {}", self.message)
     }
}
