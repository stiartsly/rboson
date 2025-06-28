use std::time::SystemTime;

use crate::{
    Id,
    Value,
    PeerInfo,
    core::Result
};

pub(crate) trait DataStorage {
    fn open(&mut self,
        path: &str
    ) -> Result<()>;

    fn close(&mut self);
    fn expire(&mut self);

    fn value(&mut self,
        id: &Id
    ) -> Result<Option<Value>>;

    fn remove_value(&mut self,
        id: &Id
    ) -> Result<()>;

    fn put_value(&mut self,
        value: &Value,
        expected_seq: Option<i32>,
        persistent: Option<bool>,
        update_last_announce: Option<bool>,
    ) -> Result<()>;

    fn put_value_and_announce(
        &mut self,
        value: &Value,
        persistent: bool
    ) -> Result<()> {
        self.put_value(&value, Some(0), Some(persistent), Some(true))
    }

    fn update_value_last_announce(
        &mut self,
        value_id: &Id
    ) -> Result<()>;

    fn persistent_values(
        &mut self,
        last_announce_before: &SystemTime
    ) -> Result<Vec<Value>>;

    fn value_ids(&mut self
    ) -> Result<Vec<Id>>;

    fn peers(&mut self,
        id: &Id,
        max_peers: usize
    ) -> Result<Vec<PeerInfo>>;

    fn peer(&mut self,
        id: &Id,
        origin: &Id
    ) -> Result<Option<PeerInfo>>;

    fn remove_peer(&mut self,
        id: &Id,
        origin: &Id
    ) -> Result<()>;

    fn put_peers(&mut self,
        peers: &[PeerInfo]
    ) -> Result<()>;

    fn put_peer(&mut self,
        peer: &PeerInfo,
        persistent: Option<bool>,
        update_last_announce: Option<bool>
    ) -> Result<()>;

    fn put_peer_and_announce(&mut self,
        peer: &PeerInfo,
        persistent: bool
    ) -> Result<()> {
        self.put_peer(peer, Some(persistent), Some(true))
    }

    fn update_peer_last_announce(&mut self,
        peer_id: &Id,
        origin: &Id
    ) -> Result<()>;

    fn persistent_peers(&mut self,
        last_announce_before: &SystemTime
    ) -> Result<Vec<PeerInfo>>;

    fn peer_ids(&mut self
    ) -> Result<Vec<Id>>;
}
