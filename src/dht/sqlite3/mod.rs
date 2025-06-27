pub(crate) mod models;
pub(crate) mod schema;
mod sql;

use crate::dht::sqlite3::models::{
    Valore,
    NewValore,
    Peer,
    NewPeer
};

use crate::dht::sqlite3::schema::valores::{
    dsl::valores,
    id          as val_id,
    persistent  as val_persistent,
    timestamp   as val_timestamp,
    announced   as val_announced,
};

use crate::dht::sqlite3::schema::peers::{
    dsl::peers,
    id          as peer_id,
    persistent  as peer_persistent,
    timestamp   as peer_timestamp,
    origin      as peer_origin,
    announced   as peer_announced
};

use diesel::prelude::*;
use diesel::result::Error;

pub(crate) fn user_version(
    conn: &mut SqliteConnection
) -> i32 {
    let result = diesel::sql_query(sql::GET_USER_VERSION)
        .execute(conn);

    match result {
        Ok(ver) => ver as i32,
        Err(_) => 0,
    }
}

pub(crate) fn drop_tbs(
    conn: &mut SqliteConnection
) -> bool {
    diesel::sql_query(sql::DROP_VALUES_TABLE).execute(conn).is_ok()     &&
    diesel::sql_query(sql::DROP_VALUES_INDEX).execute(conn).is_ok()     &&
    diesel::sql_query(sql::DROP_PEERS_TABLE).execute(conn).is_ok()      &&
    diesel::sql_query(sql::DROP_PEERS_INDEX).execute(conn).is_ok()      &&
    diesel::sql_query(sql::DROP_PEERS_ID_INDEX).execute(conn).is_ok()
}

pub(crate) fn create_tbs(
    conn: &mut SqliteConnection
) -> bool {
    diesel::sql_query(sql::SET_USER_VERSION).execute(conn).is_ok()      &&
    diesel::sql_query(sql::CREATE_VALUES_TABLE).execute(conn).is_ok()   &&
    diesel::sql_query(sql::CREATE_VALUES_INDEX).execute(conn).is_ok()   &&
    diesel::sql_query(sql::CREATE_PEERS_TABLE).execute(conn).is_ok()    &&
    diesel::sql_query(sql::CREATE_PEERS_INDEX).execute(conn).is_ok()    &&
    diesel::sql_query(sql::CREATE_PEERS_ID_INDEX).execute(conn).is_ok()
}

// --------------------------------------------------------
// "SELECT * from valores WHERE id = ? and timestamp >= ?"
// --------------------------------------------------------
pub(crate) fn get_value(
    conn: &mut SqliteConnection,
    id: &[u8],
    before: i64
) -> Result<Option<Valore>, Error> {
    valores.find(id)
        .filter(val_timestamp.ge(before))
        .select(Valore::as_select())
        .load(conn)
        .and_then(|mut v| Ok(v.pop()))
}

// -----------------------------------------------------------------------
// "INSERT INTO valores(\
// id, persistent, publicKey, privateKey, recipient, nonce,\
// signature, sequenceNumber, data, timestamp, announced) \
// VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
// publicKey=excluded.publicKey, privateKey=excluded.privateKey, \
// recipient=excluded.recipient, nonce=excluded.nonce, \
// signature=excluded.signature, sequenceNumber=excluded.sequenceNumber, \
// data=excluded.data, timestamp=excluded.timestamp";
// ------------------------------------------------------------------------
pub(crate) fn put_value(
    conn: &mut SqliteConnection,
    v: NewValore
) -> Result<bool, Error> {
    use crate::dht::sqlite3::schema::valores;
    diesel::insert_into(valores::table)  // TOOD: Filter the existed one.
        .values(&v)
        .execute(conn)
        .and_then(|num| Ok(num > 0))
}

// -----------------------------------------------------
// "UPDATE valores \
//        SET timestamp=?, announced = ? WHERE id = ?";
// -----------------------------------------------------
pub(crate) fn update_value_last_announce(
    conn: &mut SqliteConnection,
    id: &[u8],
    timestamp: i64,
    announced: i64
) -> Result<bool, Error> {
    diesel::update(valores.find(id))
        .set((val_timestamp.eq(timestamp), val_announced.eq(announced)))
        .execute(conn)
        .and_then(|num | Ok(num > 0))
}

// ------------------------------------
// "DELETE FROM valores WHERE id = ?"
// ------------------------------------
pub(crate) fn remove_value(
    conn: &mut SqliteConnection,
    id: &[u8]
) -> Result<bool, Error> {
    diesel::delete(valores.filter(val_id.eq(id)))
        .execute(conn)
        .and_then(|deleted| Ok(deleted > 0))
}

// -------------------------------------------------------------------
// "SELECT * FROM valores WHERE persistent = true AND announced <= ?";
// -------------------------------------------------------------------
pub(crate) fn persistent_values(
    conn: &mut SqliteConnection,
    before: i64
) -> Result<Vec<Valore>, Error> {
    valores.filter(val_persistent.eq(true))
        .filter(val_announced.ge(before))
        .select(Valore::as_select())
        .load(conn)
        .and_then(|v| Ok(v))
}

// ----------------------------------------------------------
// "SELECT id from valores WHERE timestamp >= ? ORDER BY id";
// ----------------------------------------------------------
pub(crate) fn value_ids(
    conn: &mut SqliteConnection,
    before: i64
) -> Result<Vec<Vec<u8>>, Error> {
    valores.filter(val_timestamp.ge(before))
        .order(val_id)
        .select(val_id)
        .load(conn)
        .and_then(|ids| Ok(ids))
}

// ----------------------------------------------------------
// "SELECT * from peers
//       WHERE id = ? and timestamp >= ?
//       ORDER BY RANDOM() LIMIT ?";
// ----------------------------------------------------------
pub(crate) fn get_peers(
    conn: &mut SqliteConnection,
    id: &[u8],
    max_peers: i64,
    before: i64
) -> Result<Vec<Peer>, Error> {
    peers.find(id)
        .filter(peer_timestamp.ge(before))
        .order(peer_id)
        .limit(max_peers)
        .load(conn)
}

// ----------------------------------------------------------
// "SELECT * from peers
//        WHERE id = ? and origin = ? and timestamp >= ?";
// ----------------------------------------------------------
pub(crate) fn get_peer(
    conn: &mut SqliteConnection,
    id: &[u8],
    origin: &[u8],
    before: i64
) -> Result<Option<Peer>, Error> {
    peers.find(id)
        .filter(peer_origin.eq(origin))
        .filter(peer_timestamp.ge(before))
        .select(Peer::as_select())
        .load(conn)
        .and_then(|mut v| Ok(v.pop()))
}

// -----------------------------------------------------------------------------------
// static UPSERT_PEER: &str = "INSERT INTO peers(\
// id, nodeId, origin, persistent, privateKey, port, \
// alternativeURL, signature, timestamp, announced) \
// VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id, nodeId, origin) DO UPDATE SET \
// persistent=excluded.persistent, privateKey=excluded.privateKey, \
// port=excluded.port, alternativeURL=excluded.alternativeURL, \
// signature=excluded.signature, timestamp=excluded.timestamp, \
//  announced=excluded.announced";
// ------------------------------------------------------------------------------------
pub(crate) fn put_peer(
    conn: &mut SqliteConnection,
    v: NewPeer
) -> Result<bool, Error> {
    use crate::dht::sqlite3::schema::peers;
    diesel::insert_into(peers::table) // TOOD: Filter the existed one.
        .values(&v)
        .execute(conn)
        .and_then(|num| Ok(num > 0))
}

// -------------------------------------------------------------------
// "UPDATE peers \
//        SET timestamp=?, announced = ? WHERE id = ? and origin = ?";
// -------------------------------------------------------------------
pub(crate) fn update_peer_last_announce(
    conn: &mut SqliteConnection,
    id: &[u8],
    origin: &[u8],
    timestamp: i64,
    announced: i64
) -> Result<bool, Error> {
    diesel::update(peers.find(id).find(origin))
        .set((peer_timestamp.eq(timestamp), peer_announced.eq(announced)))
        .execute(conn)
        .and_then(|num | Ok(num > 0))
}
// ----------------------------------------------------------
// "DELETE FROM peers WHERE id = ? and origin = ?"
// ----------------------------------------------------------
pub(crate) fn remove_peer(
    conn: &mut SqliteConnection,
    id: &[u8],
    origin: &[u8]
) -> Result<bool, Error> {
    let filters = peers
        .filter(peer_id.eq(id))
        .filter(peer_origin.eq(origin));

    diesel::delete(filters)
        .execute(conn)
        .and_then(|deleted| Ok(deleted > 0))
}

// -----------------------------------------------------------------
// "SELECT * FROM peers WHERE persistent = true AND announced <= ?"
// -----------------------------------------------------------------
pub(crate) fn persistent_peers(
    conn: &mut SqliteConnection,
    last_annouce_before: i64
) -> Result<Vec<Peer>, Error> {
    peers.filter(peer_persistent.eq(true))
        .filter(peer_announced.ge(last_annouce_before))
        .select(Peer::as_select())
        .load(conn)
}

// -----------------------------------------------------------------
// "SELECT DISTINCT id from peers WHERE timestamp >= ? ORDER BY id";
// -----------------------------------------------------------------
pub(crate) fn peer_ids(
    conn: &mut SqliteConnection,
    before: i64
) -> Result<Vec<Vec<u8>>, Error> {
    peers.filter(peer_timestamp.ge(before))
        .order(peer_id)
        .select(peer_id)
        .load(conn)
        .and_then(|ids| Ok(ids))
}

// -----------------------------------------------------------------
// DELETE FROM valores WHERE persistent != TRUE and timestamp < ?
// -----------------------------------------------------------------
pub(crate) fn remove_expired_values(
    conn: &mut SqliteConnection,
    before: i64
) -> Result<bool, Error> {
    let filters = valores
        .filter(val_persistent.ne(true))
        .filter(val_timestamp.le(before));

    diesel::delete(filters)
        .execute(conn)
        .and_then(|deleted| Ok(deleted > 0))
}

// -----------------------------------------------------------------
// DELETE FROM peers WHERE persistent != TRUE and timestamp < ?
// -----------------------------------------------------------------
pub(crate) fn remove_expired_peers(
    conn: &mut SqliteConnection,
    before: i64
) -> Result<bool, Error> {
    let filters = peers
        .filter(peer_persistent.ne(true))
        .filter(peer_timestamp.le(before));

    diesel::delete(filters)
        .execute(conn)
        .and_then(|deleted| Ok(deleted > 0))
}
