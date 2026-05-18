use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct StateError {
    message: String
}

impl StateError {
    pub fn new(message: String) -> Box<Self> {
        Box::new(Self { message })
    }
}

impl Error for StateError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateError: {}", self.message)
     }
}
