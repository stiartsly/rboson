use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct BeforeValidPeriodError {
    message: String
}

impl BeforeValidPeriodError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self { message: message.into() })
    }
}

impl Error for BeforeValidPeriodError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for BeforeValidPeriodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BeforeValidPeriodError: {}", self.message)
     }
}
