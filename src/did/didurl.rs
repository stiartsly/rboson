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
    scheme      : String,
    method      : String,
    id          : Id,
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
            scheme  : DID_SCHEME.to_string(),
            method  : DID_METHOD.to_string(),
            id      : id.clone(),
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
        println!("parts.len() = {}", parts.len());
        if parts.len() != 3 {
            return Err(Error::Malformed(format!("Invalid DIDUrl {}, must contain scheme, method and method-specific-id", trimmed)));
        }

        let scheme = parts[0].to_lowercase();
        if scheme != DID_SCHEME {
            return Err(Error::Malformed(format!("Invalid DIDUrl scheme: {}", scheme)));
        }

        let method = parts[1];
        if method != DID_METHOD {
            return Err(Error::Malformed(format!("Unsupported DIDUrl method: {}", method)));
        }

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
            scheme: DID_SCHEME.into(),
            method: DID_METHOD.into(),
            id,
            path,
            query,
            fragment
        })
    }

    pub fn parse_with_id(id: &Id, spec: &str) -> Result<Self> {
        let mut did_url = Self::parse(spec)?;
        did_url.id = id.clone();
        Ok(did_url)
    }

    pub fn create(url: &str) -> Result<Self> {
        Self::parse(url)
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn id(&self) -> &Id {
        &self.id
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
        self.scheme == other.scheme &&
        self.method == other.method &&
        self.id == other.id &&
        self.path == other.path &&
        self.query == other.query &&
        self.fragment == other.fragment
    }
}

impl fmt::Display for DIDUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.scheme, self.method, self.id)?;
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
