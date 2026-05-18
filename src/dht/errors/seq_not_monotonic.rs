use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct SeqNotMonotonic {}

impl Error for SeqNotMonotonic {
    fn description(&self) -> &str {
        "Sequence number not monotonic"
    }
}

impl SeqNotMonotonic {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}

impl fmt::Display for SeqNotMonotonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqNotMonotonic: Sequence number not monotonic")
     }
}

unsafe impl Sync for SeqNotMonotonic {}
unsafe impl Send for SeqNotMonotonic {}

