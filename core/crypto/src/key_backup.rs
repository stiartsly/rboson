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

//! The backup of a user key, in the forms every Boson app and the user portal share, as the Java
//! SDK's `KeyBackup`: the 64-byte private key (seed || public key) as Base58, alone or in the key
//! file. There is no word phrase: the key itself is the backup.
//!
//! The key is shown, written down and checked in groups of [`GROUP_SIZE`] characters, [`ROW_SIZE`]
//! groups a row, each row led by the number of its first group (1, 6, 11, ...). A backup is checked
//! by asking for two of its groups by number, not by retyping all of it.
//!
//! A private key is never a Boson string: it is shown and scanned as bare Base58.

use std::fmt;

use rand::RngExt;

use crate::{errors::ArgumentError, signature, Result};

/// The key file's name, as the apps and the user portal name it.
pub const FILE_NAME: &str = "boson-user-key.txt";

/// The key's characters are shown, read and checked in groups of this size.
pub const GROUP_SIZE: usize = 4;

/// How many groups a row of the key shows.
pub const ROW_SIZE: usize = 5;

const PRIVATE_KEY_LINE: &str = "privateKey:";

/// A row of the key: the number of its first group (1-based), and its groups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Row {
    number: usize,
    groups: Vec<String>,
}

impl Row {
    /// The number of the row's first group, 1-based.
    pub fn number(&self) -> usize {
        self.number
    }

    /// The groups of this row.
    pub fn groups(&self) -> &[String] {
        &self.groups
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.number, self.groups.join(" "))
    }
}

/// A check that the user saved the key, without retyping all of it: two different groups, by their
/// 0-based position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Challenge {
    first: usize,
    second: usize,
}

impl Challenge {
    /// A challenge for the groups at `first` and `second`, 0-based.
    pub fn new(first: usize, second: usize) -> Self {
        Self { first, second }
    }

    /// The first group asked for, 0-based.
    pub fn first(&self) -> usize {
        self.first
    }

    /// The second group asked for, 0-based.
    pub fn second(&self) -> usize {
        self.second
    }

    /// The number the user sees for a group ("Group 7"), as the rows of the key count them.
    pub fn number(&self, group: usize) -> usize {
        group + 1
    }
}

impl fmt::Display for Challenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Challenge({}, {})", self.first, self.second)
    }
}

/// The Base58 text of a private key.
pub fn encode(private_key: &signature::PrivateKey) -> String {
    private_key.to_base58()
}

/// The key split into groups of [`GROUP_SIZE`] characters, the last one possibly shorter.
pub fn groups(base58: &str) -> Vec<String> {
    let chars: Vec<char> = base58.chars().collect();
    chars.chunks(GROUP_SIZE).map(|group| group.iter().collect()).collect()
}

/// The key in rows of [`ROW_SIZE`] groups, numbered 1, 6, 11, ... by their first group.
pub fn rows(base58: &str) -> Vec<Row> {
    groups(base58)
        .chunks(ROW_SIZE)
        .enumerate()
        .map(|(index, groups)| Row {
            number: index * ROW_SIZE + 1,
            groups: groups.to_vec(),
        })
        .collect()
}

/// The rows as text: the number right-aligned, two spaces, then the groups.
pub fn grid_lines(base58: &str) -> Vec<String> {
    let rows = rows(base58);
    let Some(last) = rows.last() else {
        return Vec::new();
    };
    let width = last.number.to_string().len();
    rows.iter()
        .map(|row| format!("{:>width$}  {}", row.number, row.groups.join(" ")))
        .collect()
}

/// The key file, exactly as the user portal writes it, so either can read the other's: the key on
/// its `privateKey:` line (the only line either reads), and below it the same key in numbered
/// groups, for checking or copying it by hand.
pub fn key_file_text(private_key_base58: &str, user_id: &str) -> String {
    let mut text = String::with_capacity(512);
    text.push_str("Boson User Identity Key\n\n");
    text.push_str(&format!("{PRIVATE_KEY_LINE} {private_key_base58}\n"));
    text.push_str(&format!("publicKey:  {user_id}\n\n"));
    text.push_str("The private key in numbered groups of four, to check or copy by hand:\n\n");
    for line in grid_lines(private_key_base58) {
        text.push_str(&line);
        text.push('\n');
    }
    text.push('\n');
    text.push_str("Keep your private key secret. Anyone with this value can act as you.\n");
    text
}

/// Reads a key from what the user pasted, scanned, opened or typed: a key file (its `privateKey:`
/// line), or the key itself as Base58 or hex (`0x` optional). Spaces and line breaks are ignored,
/// and so are the row numbers of the key as the apps show it. Returns the validated private key.
pub fn parse(text: &str) -> Result<signature::PrivateKey> {
    let mut source = text;
    for line in text.split("\r\n").flat_map(|l| l.split(['\r', '\n'])) {
        let trimmed = java_trim(line);
        if let Some(rest) = trimmed.strip_prefix(PRIVATE_KEY_LINE) {
            source = rest;
            break;
        }
    }

    let value = without_row_numbers(source).concat();
    if value.is_empty() {
        return Err(ArgumentError::new("Empty key"));
    }

    let bytes = decode(&value)?;
    if bytes.len() != signature::PrivateKey::BYTES {
        return Err(ArgumentError::new(format!(
            "A user key is {} bytes, not {}",
            signature::PrivateKey::BYTES,
            bytes.len()
        )));
    }

    // The key is seed || public key. Boson builds the pair from the seed alone, so check here that
    // the public half is the seed's own: a damaged backup must not pass for a different identity.
    let derived = signature::KeyPair::try_from_seed(&bytes[..signature::KeyPair::SEED_BYTES])?;
    if derived.public_key().as_bytes() != &bytes[signature::KeyPair::SEED_BYTES..] {
        return Err(ArgumentError::new("The key's two halves don't belong together"));
    }
    Ok(derived.to_private_key())
}

// A 64-byte key is 128 hex characters; its Base58 is about 88, so the length tells them apart.
fn decode(value: &str) -> Result<Vec<u8>> {
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if hex.len() == 128 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return hex::decode(hex).map_err(|_| ArgumentError::new("Not a Base58 or hex key").into());
    }

    bs58::decode(value)
        .into_vec()
        .map_err(|_| ArgumentError::new("Not a Base58 or hex key").into())
}

// The words of a typed key, without the numbers of its rows if it was copied from the grid: a word is
// taken for a row number only where the grid puts one, before the first group of each row, and only
// if it is that row's number. A group is never that short, except the key's last, so no part of the
// key is dropped, though the digits 1 to 9 are Base58 too.
fn without_row_numbers(text: &str) -> Vec<&str> {
    let words: Vec<&str> = java_trim(text)
        .split(is_java_space)
        .filter(|word| !word.is_empty())
        .collect();

    let mut kept = Vec::with_capacity(words.len());
    let mut groups = 0;
    for (i, word) in words.iter().enumerate() {
        let at_row_start = groups % ROW_SIZE == 0 && i < words.len() - 1;
        if at_row_start && word.chars().count() < GROUP_SIZE && *word == (groups + 1).to_string() {
            continue;
        }
        kept.push(*word);
        groups += 1;
    }
    kept
}

/// Picks two different full groups of the key to ask for.
pub fn challenge(base58: &str) -> Result<Challenge> {
    challenge_with(base58, &mut rand::rng())
}

/// Picks two different full groups of the key to ask for, with `random` as the source of the choice.
pub fn challenge_with<R: rand::Rng + ?Sized>(base58: &str, random: &mut R) -> Result<Challenge> {
    let full = base58.chars().count() / GROUP_SIZE;
    if full < 2 {
        return Err(ArgumentError::new("Key too short"));
    }

    let first = random.random_range(0..full);
    let mut second = random.random_range(0..full - 1);
    if second >= first {
        second += 1;
    }
    Ok(if first < second {
        Challenge::new(first, second)
    } else {
        Challenge::new(second, first)
    })
}

/// Whether the two answers match the challenged groups. Base58 is case-sensitive; spaces are ignored.
pub fn verify(base58: &str, challenge: &Challenge, first_answer: &str, second_answer: &str) -> bool {
    let groups = groups(base58);
    let answered = |index: usize, answer: &str| {
        groups.get(index).is_some_and(|group| *group == clean(answer))
    };
    answered(challenge.first, first_answer) && answered(challenge.second, second_answer)
}

fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

// Java's String.trim() and regex \s, so text splits into the same words as in the Java SDK.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

fn is_java_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r')
}
