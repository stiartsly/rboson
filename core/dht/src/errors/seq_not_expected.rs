use std::{error::Error, fmt};

#[derive(Debug)]
pub struct SeqNotExpected;

impl SeqNotExpected {
    pub fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl Error for SeqNotExpected {}

impl fmt::Display for SeqNotExpected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sequence number not expected")
    }
}
