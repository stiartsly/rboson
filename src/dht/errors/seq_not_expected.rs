use std::{
    fmt,
    error::Error
};

#[derive(Debug)]
pub struct SeqNotExpected {
    error_string: &'static str
}

impl Error for SeqNotExpected {
    fn description(&self) -> &str {
        self.error_string
    }
}

impl SeqNotExpected {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            error_string: "sequence number not expected"
        })
    }
}

impl fmt::Display for SeqNotExpected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error: {}", self.error_string)
     }
}
