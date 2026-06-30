use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct NotOwnerError {}

impl Error for NotOwnerError {
    fn description(&self) -> &str {
        "Not owner of the peer"
    }
}

impl NotOwnerError {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}

impl fmt::Display for NotOwnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error: Not owner of the peer")
     }
}
