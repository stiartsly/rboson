use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ImmutableSubstitutionError;

impl ImmutableSubstitutionError {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl Error for ImmutableSubstitutionError {}

impl fmt::Display for ImmutableSubstitutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Not owner of the peer")
    }
}
