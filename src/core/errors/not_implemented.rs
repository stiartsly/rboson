use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct NotImplementedError {
    message: String
}

impl NotImplementedError {
    pub fn new(message: String) -> Box<Self> {
        Box::new(Self { message })
    }
}

impl Error for NotImplementedError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for NotImplementedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NotImplementedError: {}", self.message)
    }
}
