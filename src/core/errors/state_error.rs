use std::{
    fmt,
    error::Error as StdError,
};

#[derive(Debug)]
pub struct StateError {
    message: String
}

impl StateError {
    pub fn new(message: impl Into<String>) -> Box<dyn StdError> {
        Box::new(Self { message: message.into() })
    }
}

impl StdError for StateError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateError: {}", self.message)
     }
}
