use std::fmt;
use std::str::FromStr;

use unicode_normalization::UnicodeNormalization;

use crate::{DID_METHOD, DID_SCHEME};
use boson_api::{
    errors::{MalformedError, StateError},
    Error, Id, Result,
};

/// A `did:boson:` URL, or a relative reference to one.
///
/// The component layout follows the Java `DIDURL` implementation:
/// `did:<method>:<method-specific-id><path>?<query>#<fragment>`.
#[derive(Debug, Clone, Eq, Hash)]
pub struct DIDUrl {
    scheme: Option<String>,
    method: Option<String>,
    id: Option<Id>,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
}

impl DIDUrl {
    pub fn new(id: &Id, path: Option<&str>, query: Option<&str>, fragment: Option<&str>) -> Self {
        Self {
            scheme: Some(DID_SCHEME.to_string()),
            method: Some(DID_METHOD.to_string()),
            id: Some(*id),
            path: normalize_path(path),
            query: normalize_delimiter(query, '?'),
            fragment: normalize_delimiter(fragment, '#'),
        }
    }

    pub fn from_id(id: &Id) -> Self {
        Self::new(id, None, None, None)
    }

    /// Parses a DID URL or a relative reference such as `/path` or `#fragment`.
    pub fn parse(did_url: &str) -> Result<Self> {
        let spec = did_url.trim();
        if spec.is_empty() {
            return Ok(Self {
                scheme: None,
                method: None,
                id: None,
                path: None,
                query: None,
                fragment: None,
            });
        }

        if spec.starts_with('/') || spec.starts_with('#') {
            let mut url = Self {
                scheme: None,
                method: None,
                id: None,
                path: None,
                query: None,
                fragment: None,
            };
            url.parse_reference(spec)?;
            return Ok(url);
        }

        let (scheme, rest) = split_component(spec, ':').ok_or_else(|| {
            MalformedError::new(format!("Missing DIDURL scheme: {spec}"))
        })?;
        if !scheme.eq_ignore_ascii_case(DID_SCHEME) {
            return Err(MalformedError::new(format!("Invalid DIDURL scheme: {scheme}")).into());
        }

        let (method, rest) = split_component(rest, ':').ok_or_else(|| {
            MalformedError::new(format!("Missing DIDURL method: {spec}"))
        })?;
        if !method.eq_ignore_ascii_case(DID_METHOD) {
            return Err(MalformedError::new(format!(
                "Unsupported DIDURL method: {method}"
            ))
            .into());
        }

        let end = rest
            .find(['/', '?', '#'])
            .unwrap_or(rest.len());
        let id_text = &rest[..end];
        if id_text.is_empty() {
            return Err(StateError::new("Missing DIDURL method specific id").into());
        }
        let id = id_text.parse::<Id>().map_err(|_| {
            MalformedError::new(format!("Invalid DIDURL method specific id: {id_text}"))
        })?;

        let mut url = Self {
            scheme: Some(DID_SCHEME.to_string()),
            method: Some(DID_METHOD.to_string()),
            id: Some(id),
            path: None,
            query: None,
            fragment: None,
        };
        url.parse_reference(&rest[end..])?;
        Ok(url)
    }

    fn parse_reference(&mut self, spec: &str) -> Result<()> {
        let mut remainder = spec;

        if let Some(index) = remainder.find('#') {
            self.fragment = normalize_delimiter(Some(&remainder[index + 1..]), '#');
            remainder = &remainder[..index];
        }

        if let Some(index) = remainder.find('?') {
            self.query = normalize_delimiter(Some(&remainder[index + 1..]), '?');
            remainder = &remainder[..index];
        }

        if !remainder.is_empty() {
            if !remainder.starts_with('/') {
                return Err(MalformedError::new(format!(
                    "Invalid DIDURL path: {remainder}"
                ))
                .into());
            }
            self.path = Some(remainder.nfc().collect());
        }

        Ok(())
    }

    /// Parses `spec` and associates it with `id` when the spec has no id.
    pub fn parse_with_id(id: &Id, spec: &str) -> Result<Self> {
        let mut did_url = Self::parse(spec)?;
        if did_url.id.is_none() {
            did_url.id = Some(*id);
            did_url.scheme = Some(DID_SCHEME.to_string());
            did_url.method = Some(DID_METHOD.to_string());
        }
        Ok(did_url)
    }

    pub fn create(url: &str) -> Result<Self> {
        Self::parse(url)
    }

    /// Converts this DID URL to a generic [`url::Url`].
    pub fn to_uri(&self) -> Result<url::Url> {
        url::Url::parse(&self.to_string()).map_err(|error| {
            boson_api::errors::MalformedError::new(format!("Invalid DIDURL: {error}")).into()
        })
    }

    pub fn scheme(&self) -> Option<&str> {
        self.scheme.as_deref()
    }

    pub fn method(&self) -> Option<&str> {
        self.method.as_deref()
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

fn split_component(value: &str, delimiter: char) -> Option<(&str, &str)> {
    value.split_once(delimiter)
}

fn normalize_delimiter(value: Option<&str>, delimiter: char) -> Option<String> {
    let value = value?;
    let value = value.strip_prefix(delimiter).unwrap_or(value);
    if value.is_empty() {
        None
    } else {
        Some(value.nfc().collect())
    }
}

fn normalize_path(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let value = if value.starts_with('/') {
        value.to_string()
    } else {
        format!("/{value}")
    };
    Some(value.nfc().collect())
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
        self.scheme == other.scheme
            && self.method == other.method
            && self.id == other.id
            && self.path == other.path
            && self.query == other.query
            && self.fragment == other.fragment
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_displays_relative_references() {
        let id = Id::random();
        let path = DIDUrl::parse(&format!("/path/to/file")).unwrap();
        assert_eq!(path.scheme(), None);
        assert_eq!(path.id(), None);
        assert_eq!(path.path(), Some("/path/to/file"));

        let url = DIDUrl::new(&id, Some("path/to/file"), Some("a=b"), Some("frag"));
        assert_eq!(url.path(), Some("/path/to/file"));
        assert_eq!(url.to_string(), format!("did:boson:{id}/path/to/file?a=b#frag"));

        let restored = DIDUrl::parse(&url.to_string()).unwrap();
        assert_eq!(restored, url);
    }
}
