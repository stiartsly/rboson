use diesel::prelude::*;
use super::schema::{
    valores,
    peers,
};

#[allow(non_snake_case)]
#[derive(Queryable, Selectable)]
#[diesel(table_name = valores)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(unused)]
pub(crate) struct Valore {
    pub(crate) id:             Vec<u8>,
    pub(crate) publicKey:      Option<Vec<u8>>,
    pub(crate) privateKey:     Option<Vec<u8>>,
    pub(crate) recipient:      Option<Vec<u8>>,
    pub(crate) nonce:          Option<Vec<u8>>,
    pub(crate) signature:      Option<Vec<u8>>,
    pub(crate) sequenceNumber: i32,
    pub(crate) data:           Vec<u8>,
    pub(crate) persistent:     bool,
    pub(crate) updated:        i64,
}

#[allow(non_snake_case)]
#[derive(Insertable, Default)]
#[diesel(table_name = valores)]
pub(crate) struct NewValore<'a> {
    pub(crate) id:             &'a [u8],
    pub(crate) publicKey:      Option<&'a [u8]>,
    pub(crate) privateKey:     Option<&'a [u8]>,
    pub(crate) recipient:      Option<&'a [u8]>,
    pub(crate) nonce:          Option<&'a [u8]>,
    pub(crate) signature:      Option<&'a [u8]>,
    pub(crate) data:           &'a [u8],
    pub(crate) sequenceNumber: i32,
    pub(crate) persistent:     bool,
    pub(crate) updated:        i64,
}

#[allow(non_snake_case)]
#[derive(Queryable, Selectable)]
#[diesel(table_name = peers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(unused)]
pub(crate) struct Peer {
    pub(crate) id:            Vec<u8>,
    pub(crate) fingerprint:   i64,
    pub(crate) privateKey:    Option<Vec<u8>>,
    pub(crate) nonce:         Vec<u8>,
    pub(crate) sequenceNumber: i32,
    pub(crate) nodeId:        Option<Vec<u8>>,
    pub(crate) nodeSignature: Option<Vec<u8>>,
    pub(crate) signature:     Vec<u8>,
    pub(crate) endpoint:      String,
    pub(crate) extra:         Option<Vec<u8>>,
    pub(crate) persistent:    bool,
    pub(crate) updated:       i64,
}

#[allow(non_snake_case)]
#[derive(Insertable, Default)]
#[diesel(table_name = peers)]
pub(crate) struct NewPeer<'a> {
    pub(crate) id:             &'a [u8],
    pub(crate) fingerprint:    i64,
    pub(crate) privateKey:     Option<&'a [u8]>,
    pub(crate) nonce:          &'a [u8],
    pub(crate) sequenceNumber: i32,
    pub(crate) nodeId:         Option<&'a [u8]>,
    pub(crate) nodeSignature:  Option<&'a [u8]>,
    pub(crate) signature:      &'a [u8],
    pub(crate) endpoint:       &'a str,
    pub(crate) extra:          Option<&'a [u8]>,
    pub(crate) persistent:     bool,
    pub(crate) updated:        i64,
}
