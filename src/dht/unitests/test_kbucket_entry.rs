use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use crate::Id;
use crate::dht::{
    rpc::{
        rpc_target::Reachability,
        rpc_server::RpcServer,
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
    fn test_default() {
        let entry = make_entry();

        assert_eq!(entry.created_time() <= entry.last_seen(), true);
        assert_eq!(entry.is_never_contacted(), true);
        assert_eq!(entry.failed_requests(), 0);
        assert_eq!(entry.is_reachable(), false);
        assert_eq!(entry.eligible_for_nodes_list(), false);
        assert_eq!(entry.eligible_for_local_lookup(), true);
        assert_eq!(entry.rtt(), RpcServer::RPC_CALL_TIMEOUT_MAX);
    }

    #[test]
    fn test_timeout() {
        let mut entry = make_entry();
        let old = SystemTime::now() - Duration::from_secs(16 * 60);
        entry.set_last_seen(old);

        assert_eq!(entry.needs_ping(), true);

        entry.on_request_sent();
        entry.on_timeout();

        assert_eq!(entry.within_backoff_window(), true);
        assert_eq!(entry.backoff_window_end().is_some(), true);
        assert_eq!(entry.needs_ping(), false);
    }

    #[test]
    fn test_responded() {
        let mut first = make_entry();
        first.on_responded(100);

        assert_eq!(first.is_reachable(), true);
        assert_eq!(first.failed_requests(), 0);
        assert_eq!(first.rtt(), 100);

        let mut second = first.clone();
        second.on_responded(40);
        first.merge(second);

        assert_eq!(first.is_reachable(), true);
        assert!(first.rtt() < 100);
        assert!(first.rtt() > 40);
    }

    #[test]
    fn test_serde_cbor() {
        let mut entry = make_entry();
        entry.set_ver(1234);
        entry.update_last_sent(SystemTime::now() - Duration::from_secs(2));
        entry.on_timeout();
        entry.on_responded(75);

        let cbor = serde_cbor::to_vec(&entry)
            .expect("Failed to serialize KBucketEntry");
        let decoded: KBucketEntry = serde_cbor::from_slice(&cbor)
            .expect("Failed to deserialize KBucketEntry");

        assert_eq!(decoded.id(), entry.id());
        assert_eq!(decoded.socket_addr(), entry.socket_addr());
        assert_eq!(decoded.failed_requests(), entry.failed_requests());
        assert_eq!(decoded.is_reachable(), entry.is_reachable());
        assert_eq!(decoded.rtt(), entry.rtt());
        assert_eq!(decoded.ni().version(), entry.ni().version());
    }
}
