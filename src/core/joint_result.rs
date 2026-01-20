use std::hash::{Hash, Hasher};
use super::Network;

#[derive(Debug, Clone)]
pub struct JointResult<T> {
    v4: Option<T>,
    v6: Option<T>,
}

impl<T> JointResult<T> {
    pub(crate) fn new() -> Self {
        Self {
            v4: None,
            v6: None
        }
    }

    pub fn v4(&self) -> Option<&T> {
        self.v4.as_ref()
    }

    pub fn v6(&self) -> Option<&T> {
        self.v6.as_ref()
    }

    pub fn value(&self, network: Network) -> Option<&T> {
        match network {
            Network::IPv4 => self.v4(),
            Network::IPv6 => self.v6(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.v4.is_none() && self.v6.is_none()
    }

    pub fn is_complete(&self) -> bool {
        self.v4.is_some() && self.v6.is_some()
    }

    pub fn has_value(&self) -> bool {
        self.v4.is_some() || self.v6.is_some()
    }

    pub(crate) fn set_value(&mut self, network: Network, value: T) {
        match network {
            Network::IPv4 => self.v4 = Some(value),
            Network::IPv6 => self.v6 = Some(value)
        }
    }
}

impl Hash for JointResult<()> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.v4.hash(state);
        self.v6.hash(state);
    }
}
