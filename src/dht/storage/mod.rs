pub(crate) mod data_storage;
pub(crate) mod sqlite_storage;
pub(crate) mod models;
mod schema;
mod sql;

use crate::dht::storage::models::{
    Valore,
    NewValore,
    Peer,
    NewPeer,
};

use crate::dht::storage::schema::valores::{
    dsl::valores,
    id          as val_id,
    persistent  as val_persistent,
    timestamp   as val_timestamp,
    announced   as val_announced,
};

use crate::dht::storage::schema::peers::{
    dsl::peers,
    id             as peer_id,
    fingerprint    as peer_fingerprint,
    persistent     as peer_persistent,
    timestamp      as peer_timestamp,
    announced      as peer_announced,
    nodeId         as peer_node_id,
    sequenceNumber as peer_seq,
};

use diesel::prelude::*;
use diesel::result::Error;

#[derive(QueryableByName)]
struct UserVersion {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    user_version: i32,
}

pub(crate) fn user_version(conn: &mut SqliteConnection) -> i32 {
    diesel::sql_query(sql::GET_USER_VERSION)
        .load::<UserVersion>(conn)
        .map(|rows| rows.first().map_or(0, |r| r.user_version))
        .unwrap_or(0)
}

pub(crate) fn drop_tbs(conn: &mut SqliteConnection) -> bool {
    diesel::sql_query(sql::DROP_VALUES_TABLE).execute(conn).is_ok()     &&
    diesel::sql_query(sql::DROP_VALUES_INDEX).execute(conn).is_ok()     &&
    diesel::sql_query(sql::DROP_PEERS_TABLE).execute(conn).is_ok()      &&
    diesel::sql_query(sql::DROP_PEERS_INDEX).execute(conn).is_ok()      &&
    diesel::sql_query(sql::DROP_PEERS_ID_INDEX).execute(conn).is_ok()
}

pub(crate) fn create_tbs(conn: &mut SqliteConnection) -> bool {
    diesel::sql_query(sql::SET_USER_VERSION).execute(conn).is_ok()      &&
    diesel::sql_query(sql::CREATE_VALUES_TABLE).execute(conn).is_ok()   &&
    diesel::sql_query(sql::CREATE_VALUES_INDEX).execute(conn).is_ok()   &&
    diesel::sql_query(sql::CREATE_PEERS_TABLE).execute(conn).is_ok()    &&
    diesel::sql_query(sql::CREATE_PEERS_INDEX).execute(conn).is_ok()    &&
    diesel::sql_query(sql::CREATE_PEERS_ID_INDEX).execute(conn).is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Value queries
// ─────────────────────────────────────────────────────────────────────────────

// SELECT * FROM valores WHERE id = ?
pub(crate) fn get_value(
    conn: &mut SqliteConnection,
    id: &[u8],
) -> Result<Option<Valore>, Error> {
    valores.find(id)
        .select(Valore::as_select())
        .load(conn)
        .and_then(|mut v| Ok(v.pop()))
}

// SELECT * FROM valores
pub(crate) fn get_values(
    conn: &mut SqliteConnection,
) -> Result<Vec<Valore>, Error> {
    valores.select(Valore::as_select()).load(conn)
}

// INSERT OR REPLACE INTO valores(...)
pub(crate) fn put_value(
    conn: &mut SqliteConnection,
    v: NewValore,
) -> Result<bool, Error> {
    use crate::dht::storage::schema::valores;
    diesel::replace_into(valores::table)
        .values(&v)
        .execute(conn)
        .and_then(|num| Ok(num > 0))
}

// UPDATE valores SET announced = ? WHERE id = ?
pub(crate) fn update_value_announced_time(
    conn: &mut SqliteConnection,
    id: &[u8],
    announced: i64,
) -> Result<bool, Error> {
    diesel::update(valores.find(id))
        .set(val_announced.eq(announced))
        .execute(conn)
        .and_then(|num| Ok(num > 0))
}

// DELETE FROM valores WHERE id = ?
pub(crate) fn remove_value(
    conn: &mut SqliteConnection,
    id: &[u8],
) -> Result<bool, Error> {
    diesel::delete(valores.filter(val_id.eq(id)))
        .execute(conn)
        .and_then(|deleted| Ok(deleted > 0))
}

// DELETE FROM valores WHERE persistent != TRUE AND timestamp < ?
pub(crate) fn remove_expired_values(
    conn: &mut SqliteConnection,
    before: i64,
) -> Result<bool, Error> {
    diesel::delete(
        valores
            .filter(val_persistent.ne(true))
            .filter(val_timestamp.le(before))
    )
    .execute(conn)
    .and_then(|deleted| Ok(deleted > 0))
}

// ─────────────────────────────────────────────────────────────────────────────
// Peer queries
// ─────────────────────────────────────────────────────────────────────────────

// SELECT * FROM peers WHERE id = ? AND fingerprint = ?
pub(crate) fn get_peer(
    conn: &mut SqliteConnection,
    id: &[u8],
    fingerprint: i64,
) -> Result<Option<Peer>, Error> {
    peers
        .filter(peer_id.eq(id))
        .filter(peer_fingerprint.eq(fingerprint))
        .select(Peer::as_select())
        .load(conn)
        .and_then(|mut v| Ok(v.pop()))
}

// SELECT * FROM peers WHERE id = ?
pub(crate) fn get_peers_by_id(
    conn: &mut SqliteConnection,
    id: &[u8],
) -> Result<Vec<Peer>, Error> {
    peers
        .filter(peer_id.eq(id))
        .select(Peer::as_select())
        .load(conn)
}

// SELECT * FROM peers WHERE id = ? AND sequenceNumber >= ? LIMIT ?
pub(crate) fn get_peers_with_seq(
    conn: &mut SqliteConnection,
    id: &[u8],
    expected_seq: i32,
    limit: i64,
) -> Result<Vec<Peer>, Error> {
    peers
        .filter(peer_id.eq(id))
        .filter(peer_seq.ge(expected_seq))
        .limit(limit)
        .select(Peer::as_select())
        .load(conn)
}

// SELECT * FROM peers WHERE id = ? AND nodeId = ?
pub(crate) fn get_peers_authenticated_by(
    conn: &mut SqliteConnection,
    id: &[u8],
    node_id: &[u8],
) -> Result<Vec<Peer>, Error> {
    peers
        .filter(peer_id.eq(id))
        .filter(peer_node_id.eq(node_id))
        .select(Peer::as_select())
        .load(conn)
}

// SELECT * FROM peers
pub(crate) fn get_peers_all(
    conn: &mut SqliteConnection,
) -> Result<Vec<Peer>, Error> {
    peers.select(Peer::as_select()).load(conn)
}

// INSERT OR REPLACE INTO peers(...)
pub(crate) fn put_peer(
    conn: &mut SqliteConnection,
    p: NewPeer,
) -> Result<bool, Error> {
    use crate::dht::storage::schema::peers;
    diesel::replace_into(peers::table)
        .values(&p)
        .execute(conn)
        .and_then(|num| Ok(num > 0))
}

// UPDATE peers SET announced = ? WHERE id = ? AND fingerprint = ?
pub(crate) fn update_peer_announced_time(
    conn: &mut SqliteConnection,
    id: &[u8],
    fingerprint: i64,
    announced: i64,
) -> Result<bool, Error> {
    diesel::update(
        peers
            .filter(peer_id.eq(id))
            .filter(peer_fingerprint.eq(fingerprint))
    )
    .set(peer_announced.eq(announced))
    .execute(conn)
    .and_then(|num| Ok(num > 0))
}

// DELETE FROM peers WHERE id = ? AND fingerprint = ?
pub(crate) fn remove_peer(
    conn: &mut SqliteConnection,
    id: &[u8],
    fingerprint: i64,
) -> Result<bool, Error> {
    diesel::delete(
        peers
            .filter(peer_id.eq(id))
            .filter(peer_fingerprint.eq(fingerprint))
    )
    .execute(conn)
    .and_then(|deleted| Ok(deleted > 0))
}

// DELETE FROM peers WHERE id = ?
pub(crate) fn remove_peers_by_id(
    conn: &mut SqliteConnection,
    id: &[u8],
) -> Result<bool, Error> {
    diesel::delete(peers.filter(peer_id.eq(id)))
        .execute(conn)
        .and_then(|deleted| Ok(deleted > 0))
}

// DELETE FROM peers WHERE persistent != TRUE AND timestamp < ?
pub(crate) fn remove_expired_peers(
    conn: &mut SqliteConnection,
    before: i64,
) -> Result<bool, Error> {
    diesel::delete(
        peers
            .filter(peer_persistent.ne(true))
            .filter(peer_timestamp.le(before))
    )
    .execute(conn)
    .and_then(|deleted| Ok(deleted > 0))
}
