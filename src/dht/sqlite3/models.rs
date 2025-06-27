use diesel::prelude::*;
use super::schema::{
    valores,
    peers
};

#[allow(non_snake_case)]
#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = valores)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct Valore {
    pub(crate) id: Vec<u8>,
    pub(crate) persistent: bool,
    pub(crate) publicKey:   Option<Vec<u8>>,
    pub(crate) privateKey:  Option<Vec<u8>>,
    pub(crate) recipient:   Option<Vec<u8>>,
    pub(crate) nonce:       Option<Vec<u8>>,
    pub(crate) signature:   Option<Vec<u8>>,
    pub(crate) sequenceNumber: i32,
    pub(crate) data: Vec<u8>,
    pub(crate) timestamp: i64,
    pub(crate) announced: i64,
}

#[allow(non_snake_case)]
#[derive(Insertable)]
#[diesel(table_name = valores)]
#[derive(Default)]
pub(crate) struct NewValore<'a> {
    pub(crate) id: &'a [u8],
    pub(crate) publicKey:   Option<&'a [u8]>,
    pub(crate) privateKey:  Option<&'a [u8]>,
    pub(crate) recipient:   Option<&'a [u8]>,
    pub(crate) nonce:       Option<&'a [u8]>,
    pub(crate) signature:   Option<&'a [u8]>,
    pub(crate) data: &'a [u8],
    pub(crate) sequenceNumber: i32,
    pub(crate) persistent: bool,
    pub(crate) timestamp: i64,
    pub(crate) announced: i64,
}

#[allow(non_snake_case)]
#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = peers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub(crate) struct Peer {
    pub(crate) id: Vec<u8>,
    pub(crate) nodeId: Vec<u8>,
    pub(crate) origin: Vec<u8>,
    pub(crate) persistent: bool,
    pub(crate) privateKey: Option<Vec<u8>>,
    pub(crate) port: i32,
    pub(crate) alternativeURL: Option<String>,
    pub(crate) signature: Vec<u8>,
    pub(crate) timestamp: i64,
    pub(crate) announced: i64
}

#[allow(non_snake_case)]
#[derive(Insertable)]
#[diesel(table_name = peers)]
#[derive(Default)]
pub(crate) struct NewPeer<'a> {
    pub(crate) id: &'a [u8],
    pub(crate) nodeId: &'a [u8],
    pub(crate) origin: &'a [u8],
    pub(crate) persistent: bool,
    pub(crate) privateKey: Option<&'a [u8],>,
    pub(crate) port: i32,
    pub(crate) alternativeURL: Option<&'a str>,
    pub(crate) signature: &'a [u8],
    pub(crate) timestamp: i64,
    pub(crate) announced: i64,
}
