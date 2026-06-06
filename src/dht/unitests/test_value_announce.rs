use std::sync::{Arc, Mutex};

use crate::{
    Id,
    Network,
    NodeInfo,
    Value,
    crypto_identity::CryptoIdentity,
};

use crate::dht::{
    dht::DHT,
    task::{
        task::{Task, State},
        closest_set::ClosestSet,
        candidate_node::CandidateNode,
        value_announce::ValueAnnounceTask,
    },
    unitests::test_utils::make_test_dht,
};

fn make_dht() -> Arc<Mutex<DHT>> {
    make_test_dht(Arc::new(CryptoIdentity::new()), Network::IPv4, "127.0.0.1")
}

fn make_value() -> Value {
    Value::packed(
        Some(Id::random()),
        None,
        None,
        None,
        vec![1, 2, 3, 4],
        5,
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
        let value = make_value();
        let dht   = make_dht();
        let task = ValueAnnounceTask::new(Arc::downgrade(&dht), value.clone(), 7);

        assert!(task.data().is_done());
        assert!(task.is_done());
    }

    #[test]
    fn test_task_with_closestset() {
        let value = make_value();
        let dht = make_dht();
        let task = ValueAnnounceTask::new(Arc::downgrade(&dht), value, -1);
        assert!(task.is_done());

        task.with_closest(make_closestset(42));
        assert!(!task.is_done());
    }

    #[test]
    fn test_cancel() {
        let value = make_value();
        let dht = make_dht();
        let mut task = ValueAnnounceTask::new(Arc::downgrade(&dht), value, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        task.set_state_if(&State::Initialized, State::Running);
        task.iterate();
        assert!(task.is_done());
        assert!(task.is_running());

        task.cancel();
        assert!(task.is_done());
        assert!(task.is_canceled());
    }

    #[test]
    fn test_complete() {
        let value = make_value();
        let dht = make_dht();
        let mut task = ValueAnnounceTask::new(Arc::downgrade(&dht), value, -1);
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
        let value = make_value();
        let dht = make_dht();
        let mut task = ValueAnnounceTask::new(Arc::downgrade(&dht), value, -1);
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
