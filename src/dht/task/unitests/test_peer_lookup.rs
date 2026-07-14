use std::{
    rc::Rc,
    cell::RefCell,
};
use crate::{
    Id,
    Network,
};
use crate::dht::{
    dht::DHT,
    task::{
        LookupTask,
        PeerLookupTask,
    }
};
use super::test_utils::make_test_dht;

fn make_dht() -> Rc<RefCell<DHT>> {
    make_test_dht(Network::IPv4, "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let target = Id::random();
        let dht    = make_dht();
        let task   = PeerLookupTask::new(dht.clone(), target.clone(), 7, 3, true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);
        assert!(task.result().is_empty());
    }
}
