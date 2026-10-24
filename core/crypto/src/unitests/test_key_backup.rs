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

// Ported from the Java SDK's KeyBackupTests.

use rand::SeedableRng;

use crate::{
    key_backup::{self, Challenge},
    signature::KeyPair,
};

struct Fixture {
    keypair: KeyPair,
    base58: String,
    user_id: String,
}

fn fixture() -> Fixture {
    let keypair = KeyPair::random();
    let base58 = key_backup::encode(keypair.private_key());
    let user_id = keypair.public_key().to_base58();
    Fixture { keypair, base58, user_id }
}

#[test]
fn key_file_reads_back_in_the_portals_own_format() {
    let f = fixture();
    let text = key_backup::key_file_text(&f.base58, &f.user_id);
    assert!(text.starts_with(&format!(
        "Boson User Identity Key\n\nprivateKey: {}\npublicKey:  {}\n",
        f.base58, f.user_id
    )));
    for line in key_backup::grid_lines(&f.base58) {
        assert!(text.contains(&format!("{line}\n")));
    }
    assert_eq!(&key_backup::parse(&text).unwrap(), f.keypair.private_key());
}

#[test]
fn rows_hold_five_groups_and_are_numbered_by_their_first_group() {
    let f = fixture();
    let rows = key_backup::rows(&f.base58);
    let mut joined = String::new();
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.number(), i * 5 + 1);
        if i < rows.len() - 1 {
            assert_eq!(row.groups().len(), 5);
        }
        row.groups().iter().for_each(|g| joined.push_str(g));
    }
    assert_eq!(joined, f.base58);
    let lines = key_backup::grid_lines(&f.base58);
    assert!(lines[0].starts_with(&format!(" 1  {} ", rows[0].groups()[0])));
    assert!(lines[lines.len() - 1].starts_with(&format!("{}  ", rows[rows.len() - 1].number())));
}

#[test]
fn key_typed_from_its_rows_reads_with_or_without_their_numbers() {
    let f = fixture();
    let key = f.keypair.private_key();
    let lines = key_backup::grid_lines(&f.base58);
    assert_eq!(&key_backup::parse(&lines.join("\n")).unwrap(), key);
    assert_eq!(&key_backup::parse(&lines.join(" ")).unwrap(), key);
    assert_eq!(&key_backup::parse(&key_backup::groups(&f.base58).join(" ")).unwrap(), key);
    let rows_without_numbers: Vec<String> =
        key_backup::rows(&f.base58).iter().map(|r| r.groups().concat()).collect();
    assert_eq!(&key_backup::parse(&rows_without_numbers.join("\r\n")).unwrap(), key);
}

#[test]
fn row_number_is_dropped_only_where_the_rows_put_one() {
    // "6" before the 6th group is the second row's number; "4" before the 4th is part of the key,
    // which then no longer reads.
    let f = fixture();
    let mut words = key_backup::groups(&f.base58);
    words.insert(5, "6".to_string());
    assert_eq!(&key_backup::parse(&words.join(" ")).unwrap(), f.keypair.private_key());
    words.insert(3, "4".to_string());
    assert!(key_backup::parse(&words.join(" ")).is_err());
}

#[test]
fn bare_key_reads_in_base58_and_in_hex() {
    let f = fixture();
    let key = f.keypair.private_key();
    assert_eq!(&key_backup::parse(&format!("  {}\n", f.base58)).unwrap(), key);
    assert_eq!(&key_backup::parse(&key.to_hexstr()).unwrap(), key);
    assert_eq!(&key_backup::parse(&key.to_hexstr()[2..]).unwrap(), key);
}

#[test]
fn key_whose_halves_dont_belong_together_is_refused() {
    let f = fixture();
    let mut mixed = f.keypair.private_key().as_bytes().to_vec();
    mixed[32..].copy_from_slice(KeyPair::random().public_key().as_bytes());
    assert!(key_backup::parse(&bs58::encode(mixed).into_string()).is_err());
}

#[test]
fn text_that_is_no_key_is_refused() {
    assert!(key_backup::parse("not a key at all").is_err());
    assert!(key_backup::parse("   ").is_err());
}

#[test]
fn groups_are_four_characters_and_cover_the_whole_key() {
    let f = fixture();
    let groups = key_backup::groups(&f.base58);
    for group in &groups[..groups.len() - 1] {
        assert_eq!(group.len(), 4);
    }
    assert_eq!(groups.concat(), f.base58);
}

#[test]
fn challenge_asks_for_two_different_full_groups_in_order() {
    let f = fixture();
    for seed in 0..200 {
        let mut random = rand::rngs::StdRng::seed_from_u64(seed);
        let c = key_backup::challenge_with(&f.base58, &mut random).unwrap();
        assert!(c.first() < c.second());
        assert!(c.second() < f.base58.len() / 4);
    }
    assert!(key_backup::challenge(&f.base58).is_ok());
    assert!(key_backup::challenge("abcdefg").is_err());
}

#[test]
fn right_groups_verify_spaces_ignored_case_kept() {
    let f = fixture();
    let c = Challenge::new(2, 9);
    let groups = key_backup::groups(&f.base58);
    let split = format!("{} {}", &groups[9][..2], &groups[9][2..]);
    assert!(key_backup::verify(&f.base58, &c, &format!(" {} ", groups[2]), &split));
    assert!(!key_backup::verify(&f.base58, &c, &groups[9], &groups[2]));
    let swapped: String = groups[2]
        .chars()
        .map(|ch| if ch.is_uppercase() { ch.to_ascii_lowercase() } else { ch.to_ascii_uppercase() })
        .collect();
    if swapped != groups[2] {
        assert!(!key_backup::verify(&f.base58, &c, &swapped, &groups[9]));
    }
}

#[test]
fn groups_are_numbered_from_one_as_the_rows_show_them() {
    let f = fixture();
    let c = Challenge::new(3, 15);
    assert_eq!(c.number(c.first()), 4);
    assert_eq!(c.number(c.second()), 16);
    assert_eq!(key_backup::groups(&f.base58)[5], key_backup::rows(&f.base58)[1].groups()[0]);
    assert_eq!(Challenge::new(3, 15), c);
}
