use std::{error::Error, fmt};

#[derive(Debug)]
pub struct NotImplemented(String);

impl NotImplemented {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for NotImplemented {}

impl fmt::Display for NotImplemented {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
