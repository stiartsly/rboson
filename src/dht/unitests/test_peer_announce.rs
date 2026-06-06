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
        task::{Task, State},
    },
    unitests::test_utils::make_test_dht,
};

fn make_dht() -> Arc<Mutex<DHT>> {
    make_test_dht(Arc::new(CryptoIdentity::new()), Network::IPv4, "127.0.0.1")
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
    let mut cn: CandidateNode = candidate.into();
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
        let dht  = make_dht();
        let task = PeerAnnounceTask::new(Arc::downgrade(&dht), peer.clone(), 7);

        assert!(task.data().is_done());
        assert!(task.is_done());
    }

    #[test]
    fn test_task_with_closestset() {
        let peer = make_peer();
        let dht = make_dht();
        let task = PeerAnnounceTask::new(Arc::downgrade(&dht), peer, -1);
        assert!(task.is_done());

        task.with_closest(make_closestset(42));
        assert!(!task.is_done());
    }

    #[test]
    fn test_cancel() {
        println!(">>>> test_cancel line:{}", line!());
        let peer = make_peer();
        let dht  = make_dht();
        let mut task = PeerAnnounceTask::new(Arc::downgrade(&dht), peer, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        println!(">>>> test_cancel line:{}", line!());
        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        println!(">>>> test_cancel line:{}", line!());
        task.set_state_if(&State::Initialized, State::Running);
        task.iterate();
        assert!(task.is_done());
        assert!(task.is_running());

        println!(">>>> test_cancel line:{}", line!());
        task.cancel();
        assert!(task.is_done());
        assert!(task.is_canceled());
    }

    #[test]
    fn test_complete() {
        let peer = make_peer();
        let dht = make_dht();
        let mut task = PeerAnnounceTask::new(Arc::downgrade(&dht), peer, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        task.set_state_if(&State::Initialized, State::Running);
        task.iterate();
        assert!(task.is_done());
        assert!(task.is_running());

        task.complete();
        assert!(task.is_done());
        assert!(task.is_completed());
    }

    #[test]
    fn test_start() {
        let peer = make_peer();
        let dht = make_dht();
        let mut task = PeerAnnounceTask::new(Arc::downgrade(&dht), peer, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        task.start();
        assert!(task.is_done());
        assert!(task.is_completed());
    }
}
