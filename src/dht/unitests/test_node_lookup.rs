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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_task() {
        let target = Id::random();
        let task = NodeLookupTask::new(make_dht(), target.clone(), true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.is_bootstrap(), false);
        assert_eq!(task.want_token(), false);
        assert_eq!(task.want_target(), false);
        assert_eq!(task.candidate_size(), 0);
    }

    #[test]
    fn test_task_with_configuration() {
        let target = Id::random();
        let mut task = NodeLookupTask::new(make_dht(), target, false);

        task.with_bootstrap(true)
            .with_want_token(true)
            .with_want_target(true);

        assert_eq!(task.is_bootstrap(), true);
        assert_eq!(task.want_token(), true);
        assert_eq!(task.want_target(), true);
    }

    #[test]
    fn test_task_with_inject_candidates() {
        let target = Id::random();
        let mut task = NodeLookupTask::new(make_dht(), target, false);
        let candidate = NodeInfo::new(
            Id::random(),
            "1.1.1.1:39001".parse().unwrap(),
        );

        task.with_inject_candidates(vec![candidate.clone()]);

        assert_eq!(task.candidate_size(), 1);

        let next = task.next_candidate().expect("candidate should be present");
        assert_eq!(next.lock().unwrap().id(), candidate.id());
    }
}
