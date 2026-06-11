use std::{fmt, error::Error as StdError};

#[derive(Debug)]
pub struct ArgumentError(String);

impl ArgumentError {
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self(message.into()))
    }
}

impl StdError for ArgumentError {}

impl fmt::Display for ArgumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Argument error: {}", self.0)
     }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argument_error() {
        let message = "Invalid argument";
        let err = ArgumentError::new(message);
        assert_eq!(format!("{}", err), format!("Argument error: {}", message));
    }
}
