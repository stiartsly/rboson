use std::{error::Error, fmt};

#[derive(Debug)]
pub struct NotOwnerError;

impl NotOwnerError {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl Error for NotOwnerError {}

impl fmt::Display for NotOwnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Not owner of the peer")
    }
}
