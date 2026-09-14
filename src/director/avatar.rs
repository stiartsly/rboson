use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Avatar {
    pub content_type: String,
    pub data: Vec<u8>,
}

impl Avatar {
    pub fn new(content_type: String, data: Vec<u8>) -> Self {
        Self { content_type, data }
    }

    pub fn content_type(&self) -> &str {
        self.content_type.as_str()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl fmt::Display for Avatar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Avatar {{ content_type: {}, size: {} }}", self.content_type, self.data.len())
    }
}
