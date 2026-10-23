use std::{error::Error, fmt};

#[derive(Debug)]
pub struct BeforeValidPeriodError(String);

impl BeforeValidPeriodError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl Error for BeforeValidPeriodError {}

impl fmt::Display for BeforeValidPeriodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
