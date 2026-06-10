use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};
use crate::Id;
use crate::dht::{
    rpc::{
        rpc_target::Reachability,
        rpc_target::NodeInfoLike,
    },
    routing::kbucket_entry::KBucketEntry,
};

fn make_entry() -> KBucketEntry {
    KBucketEntry::new(
        Id::random(),
        "127.0.0.1:39001".parse::<SocketAddr>().unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let entry = make_entry();

        assert!(entry.created_time() <= entry.last_seen());
        assert!(entry.failed_reqs() == 0);
        assert!(!entry.is_reachable());
        assert!(!entry.eligible_for_nodes_list());
        assert!(entry.eligible_for_local_lookup());
    }

    #[test]
    fn test_on_timeout() {
        let mut entry = make_entry();
        let old = SystemTime::now() - Duration::from_secs(16 * 60);
        entry.set_last_seen(old);

        assert!(entry.needs_ping());

        entry.on_request_sent();
        entry.on_timeout();

        assert!(entry.backoff_window_end().is_some());
        assert!(!entry.needs_ping());
    }

    #[test]
    fn test_merge() {
        let mut first = make_entry();
        assert!(!first.is_reachable());

        first.on_request_sent();
        first.on_timeout();

        assert!(!first.is_reachable());
        assert!(first.failed_reqs() == 1);

        let mut second = first.clone();
        second.on_responded(40);
        first.merge(second.clone());

        assert!(first.is_reachable());
        assert!(first.failed_reqs() == 0);
        assert_eq!(first.created_time(), second.created_time());
        assert_eq!(first.last_seen(), second.last_seen());
        assert_eq!(first.last_sent(), second.last_sent());
        //assert!(first.rtt() < 100);
        //assert!(first.rtt() > 40);
    }

    #[test]
    fn test_serde() {
        let mut entry = make_entry();
        entry.set_ver(1234);
        entry.update_last_sent(SystemTime::now() - Duration::from_secs(2));
        entry.on_timeout();
        entry.on_responded(75);

        let encoded = serde_cbor::to_vec(&entry)
            .expect("Failed to serialize KBucketEntry");

        println!("encoded: {:?}", encoded);
        let decoded: KBucketEntry = serde_cbor::from_slice(&encoded)
            .expect("Failed to deserialize KBucketEntry");

        assert_eq!(decoded.id(), entry.id());
        assert_eq!(decoded.socket_addr(), entry.socket_addr());
        assert_eq!(decoded.failed_reqs(), entry.failed_reqs());
        assert_eq!(decoded.is_reachable(), entry.is_reachable());
        //assert_eq!(decoded.rtt(), entry.rtt());
        assert_eq!(decoded.ni().version(), entry.ni().version());
    }
}
