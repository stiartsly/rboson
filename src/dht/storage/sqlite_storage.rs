use std::cell::UnsafeCell;
use std::time::{Duration, SystemTime};
use diesel::prelude::*;
use log::warn;

use crate::{
    as_ms,
    Id,
    Error,
    PeerInfo,
    Value,
    Result,
    errors::{StateError, ArgumentError},
};
use crate::core::cryptobox::Nonce;
use crate::dht::storage::{
    user_version,
    drop_tbs,
    create_tbs,
    get_value,
    get_values,
    put_value,
    update_value_announced_time,
    remove_value,
    remove_expired_values,
    get_peer,
    get_peers_by_id,
    get_peers_with_seq,
    get_peers_authenticated_by,
    get_peers_all,
    put_peer,
    update_peer_announced_time,
    remove_peer,
    remove_peers_by_id,
    remove_expired_peers,

    data_storage::DataStorage,
    models::{Valore, NewValore, Peer as DbPeer, NewPeer}
};

// 2 hours in milliseconds (mirrors constants::MAX_VALUE_AGE / MAX_PEER_AGE)
const DEFAULT_EXPIRY: Duration = Duration::from_secs(2 * 60 * 60);

// Convert a Diesel / Display error into a boxed crate error.
fn db_err(e: impl std::fmt::Display) -> Error {
    StateError::new(e.to_string())
}

pub(crate) struct SqliteStorage {
    connection: UnsafeCell<Option<SqliteConnection>>,
    value_expiry: Duration,
    peer_expiry: Duration,
}

impl SqliteStorage {
    pub(crate) fn new() -> Self {
        Self {
            connection: UnsafeCell::new(None),
            value_expiry: DEFAULT_EXPIRY,
            peer_expiry:  DEFAULT_EXPIRY,
        }
    }

    // SAFETY: SqliteStorage is used through a single-threaded NodeRunner;
    // the unsafe impl Send/Sync below reflects that guarantee.
    fn conn(&self) -> &mut SqliteConnection {
        unsafe { (*self.connection.get()).as_mut().unwrap() }
    }

    pub(crate) fn supports(database_uri: &str) -> bool {
        database_uri.starts_with("jdbc:sqlite:")
    }
    pub(crate) fn remove_prefix(database_uri: &str) -> &str {
        database_uri.trim_start_matches("jdbc:sqlite:")
    }
}

unsafe impl Send for SqliteStorage {}
unsafe impl Sync for SqliteStorage {}

fn valore_to_value(v: Valore) -> Value {
    Value::packed(
        v.publicKey.as_ref().map(|pk| Id::try_from(pk.as_slice()).unwrap()),
        v.recipient.as_ref().map(|r|  Id::try_from(r.as_slice()).unwrap()),
        v.nonce.as_ref().map(|n| Nonce::try_from(n.as_slice()).unwrap()),
        v.signature,
        v.data,
        v.sequenceNumber,
    )
}

fn db_peer_to_info(p: DbPeer) -> PeerInfo {
    PeerInfo::packed(
        Id::try_from(p.id.as_slice()).unwrap(),
        p.nonce,
        p.sequenceNumber,
        p.nodeId.map(|n| Id::try_from(n.as_slice()).unwrap()),
        p.nodeSignature,
        p.signature,
        p.fingerprint as u64,
        p.endpoint,
        p.extra,
    )
}

impl DataStorage for SqliteStorage {
    fn open(&mut self, path: &str) -> Result<()> {
        let conn = SqliteConnection::establish(path)
            .map_err(|e| db_err(format!("Failed to open SQLite at '{}': {}", path, e)))?;
        // SAFETY: exclusive access via &mut self
        unsafe { *self.connection.get() = Some(conn); }

        let ver = user_version(self.conn());
        if ver < 5 && !drop_tbs(self.conn()) {
            return Err(StateError::new("Failed to drop old db tables"));
        }
        if !create_tbs(self.conn()) {
            return Err(StateError::new("Failed to create db tables"));
        }
        Ok(())
    }

    fn initialize(&mut self, value_expiry: Duration, peer_expiry: Duration) -> Result<()> {
        self.value_expiry = value_expiry;
        self.peer_expiry  = peer_expiry;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        // SAFETY: exclusive access via &mut self
        unsafe { *self.connection.get() = None; }
        Ok(())
    }

    fn purge(&mut self) -> Result<()> {
        let now          = as_ms!(SystemTime::now()) as i64;
        let value_cutoff = now - self.value_expiry.as_millis() as i64;
        let peer_cutoff  = now - self.peer_expiry.as_millis() as i64;

        if let Err(e) = remove_expired_values(self.conn(), value_cutoff) {
            warn!("Purging expired values failed: {}", e);
        }
        if let Err(e) = remove_expired_peers(self.conn(), peer_cutoff) {
            warn!("Purging expired peers failed: {}", e);
        }
        Ok(())
    }

    // ── values ───────────────────────────────────────────────────────────────

    fn put_value(&mut self, value: Value, persistent: Option<bool>) -> Result<()> {
        let now      = as_ms!(SystemTime::now()) as i64;
        let value_id = value.id();
        let v = NewValore {
            id:             value_id.as_bytes(),
            publicKey:      value.public_key().map(|pk| pk.as_bytes()),
            privateKey:     value.private_key().map(|sk| sk.as_bytes()),
            recipient:      value.recipient().map(|r| r.as_bytes()),
            nonce:          value.nonce().map(|n| n.as_bytes()),
            signature:      value.signature(),
            data:           value.data(),
            sequenceNumber: value.sequence_number(),
            persistent:     persistent.unwrap_or(false),
            timestamp:      now,
            announced:      now,
        };
        put_value(self.conn(), v)
            .map(|_| ())
            .map_err(db_err)
    }

    fn get_value(&self, id: &Id) -> Result<Option<Value>> {
        get_value(self.conn(), id.as_bytes())
            .map(|opt| opt.map(valore_to_value))
            .map_err(db_err)
    }

    fn get_values(&self) -> Result<Vec<Value>> {
        get_values(self.conn())
            .map(|vs| vs.into_iter().map(valore_to_value).collect())
            .map_err(db_err)
    }

    fn update_value_announced_time(&mut self, id: &Id) -> Result<()> {
        let now = as_ms!(SystemTime::now()) as i64;
        update_value_announced_time(self.conn(), id.as_bytes(), now)
            .map(|_| ())
            .map_err(db_err)
    }

    fn remove_value(&mut self, id: &Id) -> Result<()> {
        remove_value(self.conn(), id.as_bytes())
            .map(|_| ())
            .map_err(db_err)
    }

    // ── peers ────────────────────────────────────────────────────────────────

    fn put_peer(&mut self, peer: PeerInfo, persistent: Option<bool>) -> Result<()> {
        if !peer.is_valid() {
            return Err(ArgumentError::new("peer signature validation failed"));
        }
        let now = as_ms!(SystemTime::now()) as i64;
        let p = NewPeer {
            id:             peer.id().as_bytes(),
            fingerprint:    peer.fingerprint() as i64,
            persistent:     persistent.unwrap_or(false),
            privateKey:     peer.private_key().map(|sk| sk.as_bytes()),
            nonce:          peer.nonce(),
            sequenceNumber: peer.sequence_number(),
            nodeId:         peer.nodeid().map(|n| n.as_bytes()),
            nodeSignature:  peer.node_signature(),
            signature:      peer.signature(),
            endpoint:       peer.endpoint(),
            extra:          peer.extra_data(),
            timestamp:      now,
            announced:      now,
        };
        put_peer(self.conn(), p)
            .map(|_| ())
            .map_err(db_err)
    }

    fn put_peers(&mut self, peers_in: Vec<PeerInfo>) -> Result<()> {
        for peer in peers_in {
            self.put_peer(peer, None)?;
        }
        Ok(())
    }

    fn get_peer(&self, id: &Id, fingerprint: u64) -> Result<Option<PeerInfo>> {
        get_peer(self.conn(), id.as_bytes(), fingerprint as i64)
            .map(|opt| opt.map(db_peer_to_info))
            .map_err(db_err)
    }

    fn get_peers(&self, id: &Id) -> Result<Vec<PeerInfo>> {
        get_peers_by_id(self.conn(), id.as_bytes())
            .map(|ps| ps.into_iter().map(db_peer_to_info).collect())
            .map_err(db_err)
    }

    fn get_peers_with_expected_seq(&self, id: &Id, expected_seq: i32, limit: i32) -> Result<Vec<PeerInfo>> {
        get_peers_with_seq(self.conn(), id.as_bytes(), expected_seq, limit as i64)
            .map(|ps| ps.into_iter().map(db_peer_to_info).collect())
            .map_err(db_err)
    }

    fn get_peers_authenticated_by(&self, id: &Id, node_id: &Id) -> Result<Vec<PeerInfo>> {
        get_peers_authenticated_by(self.conn(), id.as_bytes(), node_id.as_bytes())
            .map(|ps| ps.into_iter().map(db_peer_to_info).collect())
            .map_err(db_err)
    }

    fn get_peers_all(&self) -> Result<Vec<PeerInfo>> {
        get_peers_all(self.conn())
            .map(|ps| ps.into_iter().map(db_peer_to_info).collect())
            .map_err(db_err)
    }

    fn update_peer_announced_time(&mut self, id: &Id, fingerprint: u64) -> Result<()> {
        let now = as_ms!(SystemTime::now()) as i64;
        update_peer_announced_time(self.conn(), id.as_bytes(), fingerprint as i64, now)
            .map(|_| ())
            .map_err(db_err)
    }

    fn remove_peer(&mut self, id: &Id, fingerprint: u64) -> Result<()> {
        remove_peer(self.conn(), id.as_bytes(), fingerprint as i64)
            .map(|_| ())
            .map_err(db_err)
    }

    fn remove_peers(&mut self, id: &Id) -> Result<()> {
        remove_peers_by_id(self.conn(), id.as_bytes())
            .map(|_| ())
            .map_err(db_err)
    }
}
