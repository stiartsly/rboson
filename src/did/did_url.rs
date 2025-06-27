use std::fmt;
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
    pub(crate) fn new(id: &Id,
        path: Option<&str>,
        query: Option<&str>,
        fragment: Option<&str>
    ) -> Self {
        let path = path.map(|v| v.nfc().collect::<String>());
        let query = query.map(|v| {
            let q = match v.starts_with("?") {
                true => &v[1..],
                false => v,
            };
            match q.is_empty() {
                true => None,
                false => Some(q.nfc().collect::<String>())
            }
        }).unwrap_or_default();

        let fragment = fragment.map(|v| {
            let f = match v.starts_with("#") {
                true => &v[1..],
                false => v,
            };
            match f.is_empty() {
                true => None,
                false => Some(f.nfc().collect::<String>())
            }
        }).unwrap_or_default();

        Self {
            scheme: DID_SCHEME.to_string(),
            method: DID_METHOD.to_string(),
            id: id.clone(),
            path,
            query,
            fragment
        }
    }

    pub fn from_id(id: &Id) -> Self {
        Self::new(id, None, None, None)
    }

    fn scan(spec: &str, start: usize, limit: usize, delimiters: &[char]) -> usize {
        for i in start..limit {
            let ch = spec.chars().nth(i).unwrap();
            if delimiters.contains(&ch) {
                return i;
            }
        }
        limit
    }

    pub fn parse(spec: &str) -> Result<Self> {
        let trimmed: &str = spec.trim();
        if trimmed.is_empty() {
            return Err(Error::State("DIDUrl cannot be empty".into()));
        }

        let mut start = 0;
        let limit = trimmed.len();

        let ch = trimmed.chars().nth(start).unwrap();
        let delimiters = &[':', '/', '?', '#'];

        let mut id: Option<Id> = None;
        let mut path: Option<String> = None;
        let mut query: Option<String> = None;
        let mut fragment: Option<String> = None;

        if ch != '/' && ch != '#' { // not relative url or fragment/reference
            // scan scheme
            let pos = Self::scan(trimmed, start, limit, &delimiters[..]);
            if pos > start {
                let s = &trimmed[start..pos].to_lowercase();
                if s != "did" {
                    return Err(Error::Malformed(format!("Invalid DIDUrl scheme: {}", s)));
                }

                start = match trimmed.chars().nth(pos) == Some(':') {
                    true => pos + 1,
                    false => pos
                };
            } else {
                return Err(Error::Malformed("Missing DIDUrl scheme".into()));
            }

            // scan method
            let pos = Self::scan(trimmed, start, limit, &[':', '/', '?', '#']);
            if pos > start {
                let s = &trimmed[start..pos].to_lowercase();
                if s != "boson" {
                    return Err(Error::Malformed(format!("Unsupported method: {}", s)));
                }

                start = match trimmed.chars().nth(pos) == Some(':') {
                    true => pos + 1,
                    false => pos
                };
            } else {
                return Err(Error::Malformed("Missing DIDURL method".into()));
            }

            // scan method specific id
            let pos = Self::scan(trimmed, start, limit, &['/', '?', '#']);
            if pos > start {
                let s = &trimmed[start..pos];
                id = Id::try_from(s).map_err(|_|
                    Error::Malformed(format!("Invalid method specific id: {}", s))
                ).ok();
                start = pos;
            } else {
                return Err(Error::Malformed("Missing method specific id".into()));
            }
        }

        if start < limit && trimmed.chars().nth(start) == Some('/') {
            // scan path
            let pos = Self::scan(trimmed, start + 1, limit, &['?', '#']);
            path = Some(trimmed[start..pos].nfc().collect::<String>());
            start = pos;
        }

        if start < limit && trimmed.chars().nth(start) == Some('?') {
            // scan query
            let pos = Self::scan(trimmed, start + 1, limit, &['#']);
            query = if pos > start + 1 {
                Some(trimmed[start + 1..pos].nfc().collect::<String>())
            } else {
                None
            };
            start = pos;
        }

        if start < limit && trimmed.chars().nth(start) == Some('#') {
            // scan fragment
            fragment = if start + 1< limit {
                 Some(trimmed[start + 1..limit].nfc().collect::<String>())
            } else {
                None
            }
        }

        Ok(Self {
            scheme: DID_SCHEME.into(),
            method: DID_METHOD.into(),
            id: id.unwrap(),
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
            write!(f, "{}", path)?;
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
