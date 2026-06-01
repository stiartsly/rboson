use std::sync::{Arc, Mutex};

use crate::{
    Id,
    Network,
    NodeInfo,
    crypto_identity::CryptoIdentity,
};

use crate::dht::{
    dht::DHT,
    token_manager::TokenManager,
    task::{
        node_lookup::NodeLookupTask,
        lookup_task::LookupTask,
    },
    storage::{
        data_storage::DataStorage,
        sqlite_storage::SqliteStorage,
    }
};

fn make_dht() -> Arc<Mutex<DHT>> {
    let identity = Arc::new(CryptoIdentity::new());
    let tokenman = Arc::new(TokenManager::new());
    let storage: Arc<Mutex<Box<dyn DataStorage>>> = Arc::new(Mutex::new(Box::new(SqliteStorage::new())));

    DHT::new_shared(
        identity,
        Network::IPv4,
        "127.0.0.1".to_string(),
        0,
        None,
        Vec::new(),
        storage,
        tokenman,
    ).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task() {
        let target = Id::random();
        let dht = make_dht();
        let task = NodeLookupTask::new(Arc::downgrade(&dht), target.clone(), true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);

        assert!(!task.is_bootstrap());
        assert!(!task.want_token());
        assert!(!task.want_target());

        let target = Id::random();
        let mut task = NodeLookupTask::new(Arc::downgrade(&dht), target, false);

        task.with_bootstrap(true);
        task.with_want_token(true);
        task.with_want_target(true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);

        assert!(task.is_bootstrap());
        assert!(task.want_token());
        assert!(task.want_target());

        let candidate = NodeInfo::new(
            Id::random(),
            "1.1.1.1:39001".parse().unwrap(),
        );
        task.with_inject_candidates(vec![candidate.clone()]);
        assert_eq!(task.candidate_size(), 1);

        let next = task.next_candidate().expect("candidate should be present");
        let locked_next = next.lock().unwrap();
        assert_eq!(locked_next.id(), candidate.id());
        assert_eq!(locked_next.as_ref(), &candidate);
    }
}
