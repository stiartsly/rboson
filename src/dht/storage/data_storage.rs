use std::time::Duration;
use crate::{
    Id,
    Value,
    PeerInfo,
    core::Result
};

pub(crate) trait DataStorage: Send + Sync {
    fn open(&mut self,
        path: &str
    ) -> Result<()>;

    fn initialize(&mut self,
        _: Duration,
        _: Duration
    ) -> Result<()>;

    fn close(&mut self);
    fn purge(&mut self);

    // parameters listed:
    // - value: Value;
    // - persistent: Option<bool>,
    fn put_value(
        &mut self,
        _value: Value,
        _persistent: bool,
    ) -> Result<()>;

    fn get_value(
        &self,
        _value_id: &Id
    ) -> Result<Option<Value>>;

    #[allow(unused)]
    fn get_values(&self) -> Result<Vec<Value>>;

    #[allow(unused)]
    fn get_values_announced_before(
        &self,
        _persistent: bool,
        _announced_before: u64
    ) -> Result<Vec<Value>>;

    #[allow(unused)]
    fn get_values_paginated(
        &self,
        _offset: usize,
        _limit: usize
    ) -> Result<Vec<Value>>;

    fn update_value_announced_time(
        &mut self,
        _value_id: &Id
    ) -> Result<()>;

    fn remove_value(&mut self, _: &Id) -> Result<()>;

    // methods related to peer(s)
    fn put_peer(&mut self,
        _peer: PeerInfo,
        _persistent: bool,
    ) -> Result<()>;

    fn put_peers(&mut self,
        _peers: Vec<PeerInfo>,
    ) -> Result<()>;

    fn get_peer(&self,
        _id: &Id,
        _fingerprint: u64
    ) -> Result<Option<PeerInfo>>;

    fn get_peers(&self, _: &Id) -> Result<Vec<PeerInfo>>;

    fn get_peers_with_expected_seq(&self,
        _peerid: &Id,
        _expected_seq: i32,
        _limit: i32
    ) -> Result<Vec<PeerInfo>>;

    #[allow(unused)]
    fn get_peers_authenticated_by(&self,
        _peerid: &Id,
        _authenticator_nodeid: &Id
    ) -> Result<Vec<PeerInfo>>;

    #[allow(unused)]
    fn get_peers_announced_before(&self,
        _persistent: bool,
        _announced_before: u64
    ) -> Result<Vec<PeerInfo>>;

    #[allow(unused)]
    fn get_peers_paginated(&self,
        _offset: usize,
        _limit: usize
    ) -> Result<Vec<PeerInfo>>;

    #[allow(unused)]
    fn get_peers_paginated_and_announced_before(&self,
        _offset: usize,
        _limit: usize,
        _persistent: bool,
        _announced_before: u64
    ) -> Result<Vec<PeerInfo>>;

    #[allow(unused)]
    fn get_peers_all(&self) -> Result<Vec<PeerInfo>>;

    fn update_peer_announced_time(&mut self,
        _: &Id,
        _: u64
    ) -> Result<()>;

    fn remove_peer(&mut self,
        _peerid: &Id,
        _fingerprint: u64
    ) -> Result<()>;

    fn remove_peers(&mut self,
        _peerid: &Id
    ) -> Result<()>;
}

pub(crate) fn supports(database_uri: &str) -> bool {
    database_uri.starts_with("jdbc:sqlite:")
}

pub(crate) fn database_name(database_uri: &str) -> &str {
    database_uri.trim_start_matches("jdbc:sqlite:")
}
