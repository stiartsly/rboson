use std::{error::Error, fmt};

#[derive(Debug)]
pub struct ProtocolError {
    code: i32,
    message: String,
}

impl ProtocolError {
    pub fn new(code: i32, message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            code,
            message: message.into(),
        })
    }
}

impl Error for ProtocolError {}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code: {}, message: {}", self.code, self.message)
    }
}
