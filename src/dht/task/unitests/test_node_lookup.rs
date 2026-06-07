use std::sync::{Arc, Mutex};

use crate::{
    Id,
    Network,
    NodeInfo,
    crypto_identity::CryptoIdentity,
};
use crate::dht::{
    dht::DHT,
    task::{NodeLookupTask, LookupTask},
    rpc::rpc_target::NodeInfoLike,
};
use super::test_utils::make_test_dht;

fn make_dht() -> Arc<Mutex<DHT>> {
    make_test_dht(Arc::new(CryptoIdentity::new()), Network::IPv4, "127.0.0.1")
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
        assert_eq!(locked_next.ni(), candidate);
    }
}
