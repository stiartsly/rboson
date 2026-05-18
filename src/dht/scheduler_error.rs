use std::{
    fmt,
    error::Error,
};

#[derive(Debug)]
pub struct SchedulerError {
    message: String
}

impl SchedulerError {
    pub fn new(message: String) -> Box<Self> {
         Box::new(Self { message })
    }
}

impl Error for SchedulerError {
    fn description(&self) -> &str {
        &self.message
     }
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SchedulerError: {}", self.message)
     }
}