use std::fmt;
use std::str::FromStr;
use unicode_normalization::UnicodeNormalization;

use crate::{
    Id,
    Error,
    core::Result,
};

use crate::did::{
    DID_SCHEME,
    DID_METHOD
};

#[derive(Debug, Clone, Eq, Hash)]
pub struct DIDUrl {
    scheme      : Option<String>,
    method      : Option<String>,
    id          : Option<Id>,
    path        : Option<String>,
    query       : Option<String>,
    fragment    : Option<String>
}

impl DIDUrl {
    pub fn new(id: &Id,
        path: Option<&str>,
        query: Option<&str>,
        fragment: Option<&str>
    ) -> Self {
        let query = query.map(|v| {
            let q = match v.starts_with("?") {
                true => &v[1..],
                false => v,
            };
            match q.is_empty() {
                true => None,
                false => Some(q),
            }
        }).flatten();

        let fragment = fragment.map(|v| {
            let f = match v.starts_with("#") {
                true => &v[1..],
                false => v,
            };
            match f.is_empty() {
                true => None,
                false => Some(f),
            }
        }).unwrap_or_default();

        Self {
            scheme  : Some(DID_SCHEME.to_string()),
            method  : Some(DID_METHOD.to_string()),
            id      : Some(id.clone()),
            path    : path.map(|v| v.nfc().collect::<String>()),
            query   : query.map(|v| v.nfc().collect::<String>()),
            fragment: fragment.map(|v| v.nfc().collect::<String>())
        }
    }

    pub fn from_id(id: &Id) -> Self {
        Self::new(id, None, None, None)
    }

    // did:<method>:<method-specific-id><path>?<query>#<fragment>
    pub fn parse(did_url: &str) -> Result<Self>{
        let trimmed: &str = did_url.trim();
        if trimmed.is_empty() {
            return Err(Error::State("DIDUrl cannot be empty".into()));
        }

        let parts: Vec<&str> = trimmed.splitn(3, ':').collect();
        if parts.len() != 3 && parts.len() != 1 {
            return Err(Error::Malformed(format!("Invalid DIDUrl format {}, refering to the specs: <did>:<method>:<method-specific-id><path>?<query>#<fragment>", trimmed)));
        }

        let scheme = if parts.len() == 3 {
            if parts[0] != DID_SCHEME {
                return Err(Error::Malformed(format!("Invalid DIDUrl scheme: {}", parts[0])));
            }
            Some(DID_SCHEME)
        } else {
            None
        };
        let method = if parts.len() == 3 {
            if parts[1] != DID_METHOD {
                return Err(Error::Malformed(format!("Unsupported DIDUrl method: {}", parts[1])));
            }
            Some(DID_METHOD)
        } else {
            None
        };

        let mut remainder = parts[parts.len() - 1];

        // Find and remove fragment
        let fragment = match remainder.find('#') {
            Some(idx) => Some({
                let frag = &remainder[idx + 1..];
                remainder = &remainder[..idx];
                frag.nfc().collect::<String>()
            }),
            None => None,
        };

        // Find and remove query
        let query = match remainder.find('?') {
            Some(idx) => Some({
                let query = &remainder[idx + 1..];
                remainder = &remainder[..idx];
                query.nfc().collect::<String>()
            }),
            None => None
        };

        let path = match remainder.find('/') {
            Some(idx) => Some({
                let path = &remainder[idx + 1..];
                remainder = &remainder[..idx];
                path.nfc().collect::<String>()
            }),
            None => None,
        };

        let id = remainder.parse::<Id>()
            .map_err(|_| Error::Malformed(format!("Invalid DIDUrl method specific id: {}", remainder)))?;

        Ok(Self {
            scheme: scheme.map(|s| s.to_string()),
            method: method.map(|m| m.to_string()),
            id: Some(id),
            path,
            query,
            fragment
        })
    }

    pub fn parse_with_id(id: &Id, spec: &str) -> Result<Self> {
        let mut did_url = Self::parse(spec)?;
        did_url.id = Some(id.clone());
        Ok(did_url)
    }

    pub fn create(url: &str) -> Result<Self> {
        Self::parse(url)
    }

    pub fn scheme(&self) -> &str {
        self.scheme.as_ref().map_or(DID_SCHEME, |s| s.as_str())
    }

    pub fn method(&self) -> &str {
        self.method.as_ref().map_or(DID_METHOD, |m| m.as_str())
    }

    pub fn id(&self) -> Option<&Id> {
        self.id.as_ref()
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }
}

impl TryFrom<&str> for DIDUrl {
    type Error = Error;

    fn try_from(spec: &str) -> Result<Self> {
        Self::parse(spec)
    }
}

impl FromStr for DIDUrl {
    type Err = Error;

    fn from_str(spec: &str) -> Result<Self> {
        Self::parse(spec)
    }
}

impl From<&Id> for DIDUrl {
    fn from(id: &Id) -> Self {
        Self::from_id(id)
    }
}

impl PartialEq for DIDUrl {
    fn eq(&self, other: &Self) -> bool {
        self.scheme.as_deref().unwrap_or(DID_SCHEME) == other.scheme.as_deref().unwrap_or(DID_SCHEME) &&
        self.method.as_deref().unwrap_or(DID_METHOD) == other.method.as_deref().unwrap_or(DID_METHOD) &&
        self.id == other.id &&
        self.path == other.path &&
        self.query == other.query &&
        self.fragment == other.fragment
    }
}

impl fmt::Display for DIDUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = &self.scheme {
            write!(f, "{}:", scheme)?;
        }
        if let Some(method) = &self.method {
            write!(f, "{}:", method)?;
        }
        if let Some(id) = &self.id {
            write!(f, "{}", id)?;
        }
        if let Some(path) = &self.path {
            write!(f, "/{}", path)?;
        }
        if let Some(query) = &self.query {
            write!(f, "?{}", query)?;
        }
        if let Some(fragment) = &self.fragment {
            write!(f, "#{}", fragment)?;
        }
        Ok(())
    }
}
