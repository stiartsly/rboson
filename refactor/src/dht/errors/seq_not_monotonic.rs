use std::{error::Error, fmt};

#[derive(Debug)]
pub struct SeqNotMonotonic;

impl SeqNotMonotonic {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl Error for SeqNotMonotonic {}

impl fmt::Display for SeqNotMonotonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sequence number not monotonic")
    }
}
