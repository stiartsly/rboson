use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct ArgumentError {
    message: String
}

impl ArgumentError {
    pub fn new(message: String) -> Box<Self> {
         Box::new(Self { message })
    }
}

impl Error for ArgumentError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArgumentError: {}", self.message)
     }
}
