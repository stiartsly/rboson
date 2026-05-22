use std::sync::{Arc, Mutex};

use crate::{
    Id,
    Network,
    NodeInfo,
    PeerInfo,
    crypto_identity::CryptoIdentity,
};

use crate::dht::{
    dht::DHT,
    task::{
        candidate_node::CandidateNode,
        closest_set::ClosestSet,
        peer_announce::PeerAnnounceTask,
        task::Task,
    },
    token_manager::TokenManager,
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    },
};

fn make_dht() -> Arc<Mutex<DHT>> {
    let identity = Arc::new(Mutex::new(CryptoIdentity::new()));
    let storage: Arc<Mutex<Box<dyn DataStorage>>> = Arc::new(Mutex::new(Box::new(SqliteStorage::new())));
    let tokenman = Arc::new(Mutex::new(TokenManager::new()));

    let dht = Arc::new(Mutex::new(
        DHT::new(
            identity,
            Network::IPv4,
            "127.0.0.1".to_string(),
            0,
            None,
            Vec::new(),
            storage,
            tokenman,
        ).unwrap()
    ));
    dht.lock().unwrap().set_cloned(dht.clone());
    dht
}

fn make_peer() -> PeerInfo {
    PeerInfo::packed(
        Id::random(),
        vec![7; PeerInfo::NONCE_BYTES],
        5,
        None,
        None,
        vec![9; 64],
        123456,
        "127.0.0.1:39001".to_string(),
        Some(vec![1, 2, 3]),
    )
}

fn make_closestset(token: i32) -> ClosestSet {
    let target = Id::random();
    let candidate = NodeInfo::new(
        Id::random(),
        "1.1.1.1:39001".parse().unwrap(),
    );
    let mut cn = CandidateNode::from(candidate);
    cn.set_token(token);

    let mut closest = ClosestSet::new(target, 8);
    closest.add(Arc::new(Mutex::new(cn)));
    closest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let peer = make_peer();
        let task = PeerAnnounceTask::new(make_dht(), peer.clone(), 7);

        assert!(task.data().is_done());
        assert!(task.is_done());
    }

    #[test]
    fn test_task_with_closestset() {
        let peer = make_peer();
        let mut task = PeerAnnounceTask::new(make_dht(), peer, -1);
        assert!(task.is_done());

        task.with_closest(make_closestset(42));
        assert!(!task.is_done());
    }

    #[test]
    fn test_iterate() {
        let peer = make_peer();
        let mut task = PeerAnnounceTask::new(make_dht(), peer, -1);

        assert!(task.data().is_done());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(!task.is_done());

        task.iterate();
        assert!(task.is_done());
    }
}