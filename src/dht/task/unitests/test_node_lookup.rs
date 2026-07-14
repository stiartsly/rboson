use std::{
    rc::Rc,
    cell::RefCell,
};
use crate::{
    Id,
    Network,
    NodeInfo,
};
use crate::dht::{
    dht::DHT,
    task::{NodeLookupTask, LookupTask},
    rpc::rpc_target::NodeInfoLike,
};
use super::test_utils::make_test_dht;

fn make_dht() -> Rc<RefCell<DHT>> {
    make_test_dht(Network::IPv4, "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_defaults() {
        let target = Id::random();
        let dht = make_dht();
        let task = NodeLookupTask::new(dht.clone(), target.clone(), true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);

        assert!(!task.is_bootstrap());
        assert!(!task.want_token());
        assert!(!task.want_target());
    }

    #[test]
    fn test_task_with_options() {
        let target = Id::random();
        let dht = make_dht();
        let mut task = NodeLookupTask::new(dht.clone(), target.clone(), false);

        task.with_bootstrap(true);
        task.with_want_token(true);
        task.with_want_target(true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);

        assert!(task.is_bootstrap());
        assert!(task.want_token());
        assert!(task.want_target());
    }

    #[test]
    fn test_inject_candidates() {
        let target = Id::random();
        let dht = make_dht();
        let mut task = NodeLookupTask::new(dht.clone(), target, false);

        let candidate = NodeInfo::new(
            Id::random(),
            "1.1.1.1:39001".parse().unwrap(),
        );
        task.with_inject_candidates(vec![candidate.clone()]);
        assert_eq!(task.candidate_size(), 1);

        let next = task.next_candidate().expect("candidate should be present");
        let next = next.borrow();
        assert_eq!(next.id(), candidate.id());
        assert_eq!(next.socket_addr(), candidate.socket_addr());
    }
}
