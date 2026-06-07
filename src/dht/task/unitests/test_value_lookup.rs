use std::sync::{Arc, Mutex};

use crate::{
    Id,
    Network,
    crypto_identity::CryptoIdentity,
};
use crate::dht::{
    dht::DHT,
    task::{
        lookup_task::LookupTask,
        value_lookup::ValueLookupTask,
    },
};
use super::test_utils::make_test_dht;

fn make_dht() -> Arc<Mutex<DHT>> {
    make_test_dht(Arc::new(CryptoIdentity::new()), Network::IPv4, "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let target = Id::random();
        let dht = make_dht();
        let task = ValueLookupTask::new(Arc::downgrade(&dht), target.clone(), 7, true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);
        assert_eq!(task.result().is_none(), true);
    }
}
