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

use super::schema::{peers, valores};
use diesel::prelude::*;

#[allow(non_snake_case)]
#[derive(Queryable, Selectable)]
#[diesel(table_name = valores)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(unused)]
pub(crate) struct Valore {
    pub(crate) id: Vec<u8>,
    pub(crate) publicKey: Option<Vec<u8>>,
    pub(crate) privateKey: Option<Vec<u8>>,
    pub(crate) recipient: Option<Vec<u8>>,
    pub(crate) nonce: Option<Vec<u8>>,
    pub(crate) signature: Option<Vec<u8>>,
    pub(crate) sequenceNumber: i32,
    pub(crate) data: Vec<u8>,
    pub(crate) persistent: bool,
    pub(crate) updated: i64,
}

#[allow(non_snake_case)]
#[derive(Insertable, Default)]
#[diesel(table_name = valores)]
pub(crate) struct NewValore<'a> {
    pub(crate) id: &'a [u8],
    pub(crate) publicKey: Option<&'a [u8]>,
    pub(crate) privateKey: Option<&'a [u8]>,
    pub(crate) recipient: Option<&'a [u8]>,
    pub(crate) nonce: Option<&'a [u8]>,
    pub(crate) signature: Option<&'a [u8]>,
    pub(crate) data: &'a [u8],
    pub(crate) sequenceNumber: i32,
    pub(crate) persistent: bool,
    pub(crate) updated: i64,
}

#[allow(non_snake_case)]
#[derive(Queryable, Selectable)]
#[diesel(table_name = peers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(unused)]
pub(crate) struct Peer {
    pub(crate) id: Vec<u8>,
    pub(crate) fingerprint: i64,
    pub(crate) privateKey: Option<Vec<u8>>,
    pub(crate) sequenceNumber: i32,
    pub(crate) nodeId: Option<Vec<u8>>,
    pub(crate) nodeSignature: Option<Vec<u8>>,
    pub(crate) signature: Vec<u8>,
    pub(crate) endpoint: String,
    pub(crate) extra: Option<Vec<u8>>,
    pub(crate) persistent: bool,
    pub(crate) updated: i64,
}

#[allow(non_snake_case)]
#[derive(Insertable, Default)]
#[diesel(table_name = peers)]
pub(crate) struct NewPeer<'a> {
    pub(crate) id: &'a [u8],
    pub(crate) fingerprint: i64,
    pub(crate) privateKey: Option<&'a [u8]>,
    pub(crate) sequenceNumber: i32,
    pub(crate) nodeId: Option<&'a [u8]>,
    pub(crate) nodeSignature: Option<&'a [u8]>,
    pub(crate) signature: &'a [u8],
    pub(crate) endpoint: &'a str,
    pub(crate) extra: Option<&'a [u8]>,
    pub(crate) persistent: bool,
    pub(crate) updated: i64,
}
