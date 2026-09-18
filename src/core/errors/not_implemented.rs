use std::{error::Error, fmt};

#[derive(Debug)]
pub struct NotImplementedError(String);

impl NotImplementedError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for NotImplementedError {}

impl fmt::Display for NotImplementedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
