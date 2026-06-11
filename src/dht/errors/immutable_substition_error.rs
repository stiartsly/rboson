use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct ImmutableSubstitutionError {}

impl ImmutableSubstitutionError {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}

impl Error for ImmutableSubstitutionError {
    fn description(&self) -> &str {
        "Not owner of the peer"
    }
}

impl fmt::Display for ImmutableSubstitutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ImmutableSubstitutionError: Not owner of the peer")
     }
}

unsafe impl Sync for ImmutableSubstitutionError {}
unsafe impl Send for ImmutableSubstitutionError {}

