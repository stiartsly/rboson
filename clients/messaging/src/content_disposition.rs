// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::fmt;
use unicode_normalization::{
    char::is_combining_mark,
    UnicodeNormalization
};
use boson_api::errors::{ArgumentError, Result};

pub const HEADER_NAME: &str = "Content-Disposition";

/// How message content should be presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Inline,
    Attachment,
}

impl Type {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "inline" => Ok(Self::Inline),
            "attachment" => Ok(Self::Attachment),
            _ => Err(ArgumentError::new(format!(
                "Invalid content disposition type: {value}"
            ))),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inline => "inline",
            Self::Attachment => "attachment",
        })
    }
}

/// Represents the Content-Disposition header, which indicates if the content is
/// expected to be displayed inline or as an attachment,
/// potentially with a filename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDisposition {
    disposition_type: Type,
    filename: Option<String>,
    ascii_filename: Option<String>,
    rfc5987_filename: Option<String>,

}

impl ContentDisposition {
    /// An inline disposition with no filename.
    pub fn inline() -> Self {
        Self {
            disposition_type: Type::Inline,
            filename: None,
            ascii_filename: None,
            rfc5987_filename: None,
        }
    }

    /// An inline disposition carrying a filename hint.
    pub fn inline_with_name(filename: impl Into<String>) -> Self {
        Self::with_filename(Type::Inline, filename.into())
    }

    /// An attachment disposition.
    pub fn attachment(filename: impl Into<String>) -> Self {
        Self::with_filename(Type::Attachment, filename.into())
    }

    fn with_filename(disposition_type: Type, filename: String) -> Self {
        match filename.is_empty() {
            true=> Self {
                disposition_type,
                ascii_filename: None,
                rfc5987_filename: None,
                filename: None,
            },
            false=> Self {
                disposition_type,
                ascii_filename: Some(ascii_fallback(&filename)),
                rfc5987_filename: Some(encode_rfc5987(&filename)),
                filename: Some(filename),
            }
        }
    }

    pub fn parse(header: &str) -> Result<Self> {
        let mut parts = header.split(';');
        let disposition_type = Type::parse(
            &parts.next().unwrap_or_default().trim().to_ascii_lowercase(),
        )?;
        let mut ascii_filename = None;
        let mut rfc5987_filename = None;

        for part in parts {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            match name.trim().to_ascii_lowercase().as_str() {
                "filename" => {
                    ascii_filename = Some(unquote(value.trim()).to_string());
                }
                "filename*" => {
                    rfc5987_filename = Some(value.trim().to_string());
                }
                _ => {}
            }
        }

        let filename = match rfc5987_filename.as_deref() {
            Some(value) => Some(decode_rfc5987(value)?),
            _ => ascii_filename.clone(),
        };

        Ok(Self {
            disposition_type,
            ascii_filename,
            rfc5987_filename,
            filename,
        })
    }

    pub fn get_type(&self) -> Type {
        self.disposition_type
    }

    /// The filename hint, if any.
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    pub fn ascii_filename(&self) -> Option<&str> {
        self.ascii_filename.as_deref()
    }

    pub fn rfc5987_filename(&self) -> Option<&str> {
        self.rfc5987_filename.as_deref()
    }

    pub fn is_inline(&self) -> bool {
        self.disposition_type == Type::Inline
    }

    pub fn is_attachment(&self) -> bool {
        self.disposition_type == Type::Attachment
    }

    pub fn value(&self) -> String {
        format!("{}{}{}",
            self.disposition_type,
            self.ascii_filename.as_deref().map(
                |v| format!("; filename=\"{v}\"")
            ).unwrap_or("".into()),
            self.rfc5987_filename.as_deref().map(|v|
                format!("; filename*={v}")
            ).unwrap_or("".into()),
        )
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{HEADER_NAME}: {}", self.value())
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn ascii_fallback(filename: &str) -> String {
    filename
        .nfkd()
        .filter(|ch| !is_combining_mark(*ch))
        .map(|ch| if ch.is_ascii() && !ch.is_ascii_control() { ch } else { '_' })
        .collect()
}

fn encode_rfc5987(filename: &str) -> String {
    let mut encoded = String::from("UTF-8''");
    for byte in filename.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn decode_rfc5987(value: &str) -> Result<String> {
    let (charset, encoded) = value
        .split_once('\'')
        .and_then(|(charset, rest)|
            rest.strip_prefix('\'').map(|encoded| (charset, encoded))
        )
        .ok_or_else(||
            ArgumentError::new(format!("Invalid RFC 5987 content disposition: {value}"))
        )?;

    if !charset.eq_ignore_ascii_case("utf-8") {
        return Err(ArgumentError::new(format!(
            "Unsupported RFC 5987 charset: {charset}"
        )));
    }

    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos] == b'%' {
            if pos + 2 >= bytes.len() {
                return Err(ArgumentError::new(format!(
                    "Invalid RFC 5987 content disposition: {value}"
                )));
            }
            let hex = std::str::from_utf8(&bytes[pos + 1..pos + 3])
                .map_err(|_| ArgumentError::new("Invalid RFC 5987 escape"))?;
            decoded.push(u8::from_str_radix(hex, 16).map_err(|_| {
                ArgumentError::new(format!("Invalid RFC 5987 content disposition: {value}"))
            })?);
            pos += 3;
        } else {
            decoded.push(bytes[pos]);
            pos += 1;
        }
    }
    Ok(String::from_utf8(decoded).map_err(|_| {
        ArgumentError::new(format!("Invalid RFC 5987 content disposition: {value}"))
    })?)
}
