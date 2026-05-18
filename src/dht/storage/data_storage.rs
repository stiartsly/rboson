use std::time::Duration;

use crate::{
    Id, PeerInfo, Value, core::Result
};

pub(crate) trait DataStorage: Send + Sync{
    fn open(&mut self,
        path: &str
    ) -> Result<()>;

    fn initialize(&mut self,
        _: Duration,
        _: Duration
    ) -> Result<()>;

    fn close(&mut self) -> Result<()>;
    fn purge(&mut self) -> Result<()>;

    fn put_value(&mut self,
        _: Value,
        _persistent: Option<bool>
    ) -> Result<()>;

    fn get_value(&self, _: &Id) -> Result<Option<Value>>;

    fn get_values(&self)-> Result<Vec<Value>>;

    fn update_value_announced_time(&mut self,  _: &Id) -> Result<()>;

    fn remove_value(&mut self, _: &Id) -> Result<()>;

    // methods related to peer(s)

    fn put_peer(&mut self,
        _peer: PeerInfo,
        _persistent: Option<bool>
    ) -> Result<()>;

    fn put_peers(&mut self,
        _: Vec<PeerInfo>,
    ) -> Result<()>;

    fn get_peer(&self,
        _: &Id,
        _: u64
    ) -> Result<Option<PeerInfo>>;

    fn get_peers(&self, _: &Id) -> Result<Vec<PeerInfo>>;

    fn get_peers_with_expected_seq(&self,
        _: &Id,
        _: i32,
        _: i32
    ) -> Result<Vec<PeerInfo>>;

    fn get_peers_authenticated_by(&self,
        _: &Id,
        _: &Id
    ) -> Result<Vec<PeerInfo>>;

    fn get_peers_all(& self) -> Result<Vec<PeerInfo>>;

    fn update_peer_announced_time(&mut self,
        _: &Id,
        _: u64
    ) -> Result<()>;

    fn remove_peer(&mut self,
        _: &Id,
        _: u64
    ) -> Result<()>;

    fn remove_peers(&mut self, _: &Id) -> Result<()>;
}
