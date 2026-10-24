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

// pub(crate) const CURRENT_VERSION: i32 = 5;
pub(crate) const SET_USER_VERSION: &str = "PRAGMA user_version = 5";
pub(crate) const GET_USER_VERSION: &str = "PRAGMA user_version";

pub(crate) const CREATE_VALUES_TABLE: &str = "
        CREATE TABLE IF NOT EXISTS valores(\
        id BLOB NOT NULL PRIMARY KEY, \
        publicKey BLOB, \
        privateKey BLOB, \
        recipient BLOB, \
        nonce BLOB, \
        signature BLOB, \
        sequenceNumber INTEGER NOT NULL DEFAULT 0, \
        data BLOB NOT NULL, \
        persistent BOOLEAN NOT NULL DEFAULT FALSE, \
        updated INTEGER NOT NULL DEFAULT 0\
        ) WITHOUT ROWID
    ";

pub(crate) const CREATE_VALUES_INDEX: &str = "
        CREATE INDEX IF NOT EXISTS idx_valores_updated ON valores(updated)
    ";

pub(crate) const CREATE_PEERS_TABLE: &str = "
        CREATE TABLE IF NOT EXISTS peers( \
        id BLOB NOT NULL, \
        fingerprint INTEGER NOT NULL, \
        persistent BOOLEAN NOT NULL DEFAULT FALSE, \
        privateKey BLOB, \
        sequenceNumber INTEGER NOT NULL DEFAULT 0, \
        nodeId BLOB, \
        nodeSignature BLOB, \
        signature BLOB NOT NULL, \
        endpoint TEXT NOT NULL, \
        extra BLOB, \
        updated INTEGER NOT NULL DEFAULT 0, \
        PRIMARY KEY(id, fingerprint)\
        ) WITHOUT ROWID
    ";

pub(crate) const CREATE_PEERS_INDEX: &str = "
        CREATE INDEX IF NOT EXISTS idx_peers_updated ON peers(updated)
    ";

pub(crate) const CREATE_PEERS_ID_INDEX: &str = "
        CREATE INDEX IF NOT EXISTS idx_peers_id ON peers(id)
    ";

pub(crate) const DROP_VALUES_TABLE: &str = "
        DROP TABLE IF EXISTS valores
    ";

pub(crate) const DROP_VALUES_INDEX: &str = "
        DROP INDEX IF EXISTS idx_valores_timestamp
    ";

pub(crate) const DROP_PEERS_TABLE: &str = "
        DROP TABLE IF EXISTS peers
    ";

pub(crate) const DROP_PEERS_INDEX: &str = "
        DROP INDEX IF EXISTS idx_peers_timestamp
    ";

pub(crate) const DROP_PEERS_ID_INDEX: &str = "
        DROP INDEX IF EXISTS idx_peers_id
    ";
