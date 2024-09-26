use std::time::SystemTime;
use diesel::prelude::*;
use log::{debug, warn};

use crate::{
    as_millis,
    Id,
    PeerInfo,
    Value,
};

use crate::core::{
    constants,
    value::PackBuilder as ValuePackBuilder,
    peer_info::PackBuilder as PeerPackBuilder,
    signature::PrivateKey,
    cryptobox::Nonce,
    error::{Error, Result},
    data_storage::DataStorage,

};

use crate::core::sqlite3::{
    models::NewValore,
    models::NewPeer,
    user_version,
    drop_tbs,
    create_tbs,
    remove_expired_values,
    remove_expired_peers,
    get_value,
    put_value,
    update_value_last_announce,
    remove_value,
    persistent_values,
    value_ids,
    get_peers,
    get_peer,
    put_peer,
    update_peer_last_announce,
    remove_peer,
    persistent_peers,
    peer_ids
};

pub(crate) struct SqliteStorage {
    connection: Option<SqliteConnection>,
}

impl SqliteStorage {
    pub(crate) fn new() -> Self {
       Self { connection: None }
    }

    fn conn(&mut self) -> &mut SqliteConnection {
        self.connection.as_mut().unwrap()
    }
}

impl DataStorage for SqliteStorage {
    fn open(&mut self, path: &str) -> Result<()> {
        let connection = match SqliteConnection::establish(&path) {
            Ok(c) => c,
            Err(e) => return Err(Error::from(e))
        };
        self.connection = Some(connection);

        // if we change the schema,
        // we should check the user version, do the schema update,
        // then increase the user_version;
        let ver  = user_version(self.conn());
        let conn = self.connection.as_mut().unwrap();
        if ver < 4 && !drop_tbs(conn) {
            return Err(Error::State(format!("Failed to update db tables")));
        }
        if !create_tbs(conn) {
            return Err(Error::State(format!("Failed to update SQLite Text")));
        }

        Ok(())
    }
    fn close(&mut self) {
        self.connection = None;
    }

    fn expire(&mut self) {
        debug!("Remove all expired values and peers from local SQLite storage.");

        let before = millis_since_epoch() - constants::MAX_VALUE_AGE;
        remove_expired_values(self.conn(), before as i64)
            .map_err(|e| warn!("Removing expired values from SQLite storage error: {}", e))
            .ok();

        let before = millis_since_epoch() - constants::MAX_VALUE_AGE;
        remove_expired_peers(self.conn(), before as i64)
            .map_err(|e| warn!("Removing expired peers from SQLite storage error: {}", e))
            .ok();
    }

    fn value(&mut self, id: &Id) -> Result<Option<Value>> {
        let before = millis_since_epoch() - constants::MAX_VALUE_AGE;
        match get_value(self.conn(), id.as_bytes(), before as i64) {
            Ok(Some(v)) => {
                let peer = ValuePackBuilder::new(v.data)
                    .with_pk(v.publicKey  .as_ref().map(|v| Id::try_from(v.as_slice()).unwrap()))
                    .with_sk(v.privateKey .as_ref().map(|v| PrivateKey::try_from(v.as_slice()).unwrap()))
                    .with_rec(v.recipient .as_ref().map(|v| Id::try_from(v.as_slice()).unwrap()))
                    .with_nonce(v.nonce   .as_ref().map(|v| Nonce::try_from(v.as_slice()).unwrap()))
                    .with_sig(v.signature)
                    .with_seq(v.sequenceNumber)
                    .build();

                // Retaining them solely to eliminate compilation warnings.
                _ = v.id;
                _ = v.timestamp;
                _ = v.announced;
                _ = v.persistent;

                Ok(Some(peer))
            },
            Ok(None) => Ok(None),
            Err(e) => Err(Error::from(e))
        }
    }

    fn remove_value(&mut self, id: &Id) -> Result<()> {
        remove_value(self.conn(), id.as_bytes())
            .and_then(|_| Ok(()))
            .map_err(|e| Error::from(e))
    }

    fn put_value(&mut self,
        value: &Value,
        expected_seq: Option<i32>,
        persistent: Option<bool>,
        update_last_announce: Option<bool>
    ) -> Result<()> {
        if value.is_mutable() && !value.is_valid() {
            return Err(Error::Argument(format!("value signature validation failed.")));
        }
        let expected_seq = expected_seq.unwrap_or(-1);
        let value_id = value.id();
        if let Ok(Some(old)) = self.value(&value_id) {
            if old.is_mutable() {
                if !value.is_mutable() {
                    return Err(Error::Argument(format!("Can not replace mutable value with immutable is not supported")));
                }
                if old.private_key().is_some() && !value.private_key().is_some() {
                    return Err(Error::Argument(format!("Not the owner of value")));
                }
                if value.sequence_number() < old.sequence_number() {
                    return Err(Error::Argument(format!("Sequence number less than current")));
                }
                if expected_seq >= 0 &&
                    old.sequence_number() >= 0 &&
                    old.sequence_number() != expected_seq {
                    return Err(Error::Argument(format!("CAS failure")));
                }
            }
        }

        let mut v = NewValore::default();
        v.publicKey  = value.public_key()   .map(|v| v.as_bytes());
        v.privateKey = value.private_key()  .map(|v| v.as_bytes());
        v.recipient  = value.recipient()    .map(|v| v.as_bytes());
        v.nonce      = value.nonce()        .map(|v| v.as_bytes());
        v.signature  = value.signature();
        v.data       = value.data().as_ref();
        v.id         = value_id.as_bytes();
        v.persistent = persistent.unwrap_or(false);
        v.sequenceNumber = value.sequence_number();

        v.timestamp  = millis_since_epoch() as i64;
        v.announced  = if update_last_announce.unwrap_or(false) { v.timestamp } else { 0 };

        put_value(self.conn(), v)
            .and_then(|_| Ok(()))
            .map_err(|e| Error::from(e))
    }

    fn update_value_last_announce(&mut self, id: &Id) -> Result<()> {
        let timestamp = millis_since_epoch();
        update_value_last_announce(self.conn(),
                id.as_bytes(),
                timestamp as i64,
                timestamp as i64)
            .and_then(|_| Ok(()))
            .map_err(|e| Error::from(e))
    }

    fn persistent_values(&mut self, before: &SystemTime) -> Result<Vec<Value>> {
        let before = as_millis!(before) as i64;
        let result = persistent_values(self.conn(), before);
        let values = match result {
            Ok(v) => v,
            Err(e) => return Err(Error::from(e))
        };

        let values = values.into_iter()
            .filter_map(|v| {
                let value = ValuePackBuilder::new(v.data)
                    .with_pk(v.publicKey  .as_ref().map(|v| Id::try_from(v.as_slice()).unwrap()))
                    .with_sk(v.privateKey .as_ref().map(|v| PrivateKey::try_from(v.as_slice()).unwrap()))
                    .with_rec(v.recipient .as_ref().map(|v| Id::try_from(v.as_slice()).unwrap()))
                    .with_nonce(v.nonce   .as_ref().map(|v| Nonce::try_from(v.as_slice()).unwrap()))
                    .with_sig(v.signature)
                    .with_seq(v.sequenceNumber)
                    .build();

                // Retaining them solely to eliminate compilation warnings.
                _ = v.id;
                _ = v.timestamp;
                _ = v.announced;
                _ = v.persistent;

                Some(value)
            }).collect();

        Ok(values)
    }

    fn value_ids(&mut self) -> Result<Vec<Id>> {
        let timestamp = millis_since_epoch() - constants::MAX_VALUE_AGE;
        value_ids(self.conn(), timestamp as i64)
            .and_then(|v| {
                let ids = v.iter()
                    .map(|id| Id::try_from(id.as_slice()).unwrap())
                    .collect();
                Ok(ids)
            }).map_err(|e| Error::from(e))
    }

    fn peers(&mut self, peer_id: &Id, max_peers: usize) -> Result<Vec<PeerInfo>> {
        let timestamp = millis_since_epoch() - constants::MAX_VALUE_AGE;
        let result = get_peers( self.conn(),
            peer_id.as_bytes(),
            max_peers as i64,
            timestamp as i64
        );

        let peers = match result {
            Ok(v) => v,
            Err(e) => return Err(Error::from(e))
        };

        let peers = peers.into_iter().filter_map(|v| {
            let nodeid = Id::try_from(v.nodeId.as_slice()).unwrap();
            let peer = PeerPackBuilder::new(nodeid)
                .with_peerid(Some(Id::try_from(v.id.as_slice()).unwrap()))
                .with_sk(v.privateKey.as_ref().map(|v| PrivateKey::try_from(v.as_slice()).unwrap()))
                .with_port(v.port as u16)
                .with_sig(Some(v.signature))
                .with_url(v.alternativeURL)
                .with_origin(match v.origin == v.nodeId {
                    true => None,
                    false => Some(Id::try_from(v.origin.as_slice()).unwrap())
                }).build();

            // Retaining them solely to eliminate compilation warnings.
            _ = v.timestamp;
            _ = v.announced;
            _ = v.persistent;

            Some(peer)
        }).collect();

        Ok(peers)
    }

    fn peer(&mut self, id: &Id, origin: &Id) -> Result<Option<PeerInfo>> {
        let timestamp = millis_since_epoch() - constants::MAX_VALUE_AGE;
        match get_peer(self.conn(), id.as_bytes(), origin.as_bytes(), timestamp as i64) {
            Ok(Some(v)) => {
                let nodeid = Id::try_from(v.nodeId.as_slice()).unwrap();
                let peer = PeerPackBuilder::new(nodeid)
                    .with_peerid(Some(Id::try_from(v.id.as_slice()).unwrap()))
                    .with_sk(v.privateKey.as_ref().map(|v| PrivateKey::try_from(v.as_slice()).unwrap()))
                    .with_port(v.port as u16)
                    .with_sig(Some(v.signature))
                    .with_url(v.alternativeURL)
                    .with_origin(match v.origin == v.nodeId {
                        true => None,
                        false => Some(Id::try_from(v.origin.as_slice()).unwrap())
                    })
                    .build();
                Ok(Some(peer))
            },
            Ok(None) => Ok(None),
            Err(e) => return Err(Error::from(e))
        }
    }

    fn remove_peer(&mut self, peer_id: &Id, origin: &Id) -> Result<()> {
        remove_peer(self.conn(), peer_id.as_bytes(), origin.as_bytes())
            .and_then(|_| Ok(()))
            .map_err(|e| Error::from(e))
    }

    fn put_peers(&mut self, _peer: &[PeerInfo]) -> Result<()> {
        //TOOD: unimplemented!()
        Ok(())
    }

    fn put_peer(&mut self,
        peer: &PeerInfo,
        persistent: Option<bool>,
        update_last_announce: Option<bool>
    ) -> Result<()> {
        if !peer.is_valid() {
            return Err(Error::Argument(format!("peer signature validation failed.")));
        }

        let mut p = NewPeer::default();
        p.id        = peer.id().as_bytes();
        p.nodeId    = peer.nodeid().as_bytes();
        p.origin    = peer.origin().as_bytes();
        p.privateKey= peer.private_key().map(|v|v.as_bytes());
        p.persistent= persistent.unwrap_or(false);
        p.port      = peer.port() as i32;
        p.alternativeURL = peer.alternative_url();
        p.signature = peer.signature();

        p.timestamp = millis_since_epoch() as i64;
        p.announced = if update_last_announce.unwrap_or(false) { p.timestamp } else { 0 };

        put_peer(self.conn(), p)
            .and_then(|_| Ok(()))
            .map_err(|e| Error::from(e))
    }

    fn update_peer_last_announce(&mut self, target: &Id, origin: &Id) -> Result<()> {
        let timestamp = millis_since_epoch() as i64;
        update_peer_last_announce(self.conn(),
                target.as_bytes(),
                origin.as_bytes(),
                timestamp,
                timestamp)
            .and_then(|_| Ok(()))
            .map_err(|e| Error::from(e))
    }

    fn persistent_peers(&mut self, before: &SystemTime) -> Result<Vec<PeerInfo>> {
        let before = as_millis!(before) as i64;
        let result = persistent_peers(self.conn(), before);
        let result = match result {
            Ok(v) => v,
            Err(e) => return Err(Error::from(e))
        };

        let peers = result.into_iter()
            .filter_map(|v| {
                let nodeid = Id::try_from(v.nodeId.as_slice()).unwrap();
                let peer = PeerPackBuilder::new(nodeid)
                    .with_peerid(Some(Id::try_from(v.id.as_slice()).unwrap()))
                    .with_sk(v.privateKey.map(|v| PrivateKey::try_from(v.as_slice()).unwrap()))
                    .with_port(v.port as u16)
                    .with_sig(Some(v.signature))
                    .with_url(v.alternativeURL)
                    .with_origin(match v.origin == v.nodeId {
                        true => None,
                        false => Some(Id::try_from(v.origin.as_slice()).unwrap())
                    })
                    .build();

                // Retaining them solely to eliminate compilation warnings.
                _ = v.timestamp;
                _ = v.announced;
                _ = v.persistent;

                Some(peer)
            }).collect();

        Ok(peers)
    }

    fn peer_ids(&mut self) -> Result<Vec<Id>> {
        let timestamp  = millis_since_epoch() - constants::MAX_VALUE_AGE;
        peer_ids(self.conn(), timestamp as i64)
            .and_then(|v| {
                let ids = v.iter()
                    .map(|id| Id::try_from(id.as_slice()).unwrap())
                    .collect();
                Ok(ids)
            }).map_err(|e| Error::from(e))
    }
}

#[inline(always)]
fn millis_since_epoch() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
