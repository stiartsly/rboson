use std::fmt;

const ID_BYTES: usize = 32;

pub struct Id {
    bytes: [u8; ID_BYTES],
}

impl Id {
    pub fn zero() -> Self {
        Id {
            bytes: [0; ID_BYTES],
        }
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.bytes {
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}
