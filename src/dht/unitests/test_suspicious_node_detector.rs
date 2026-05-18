use std::net::SocketAddr;
use std::time::Duration;

use crate::core::{
    DefaultSuspiciousNodeDetector,
    Id,
    SuspiciousNodeDetector,
};

#[test]
fn test_observe_and_last_known_id() {
    let mut detector = DefaultSuspiciousNodeDetector::default();
    let addr = "127.0.0.1:39001".parse::<SocketAddr>().unwrap();
    let id = Id::random();

    detector.observe(addr, id);

    assert_eq!(detector.is_suspicious(&addr), false);
    assert_eq!(detector.last_known_id(&addr), Some(&id));
    assert_eq!(detector.is_suspicious_with_expected(&addr, Some(&id)), false);
    assert_eq!(detector.is_suspicious_with_expected(&addr, Some(&Id::random())), true);
}

#[test]
fn test_observation_hits_make_node_suspicious_before_ban() {
    let mut detector = DefaultSuspiciousNodeDetector::new(
        Duration::from_secs(60),
        32,
        Duration::from_secs(60),
    );
    let addr = "127.0.0.1:39002".parse::<SocketAddr>().unwrap();

    detector.observe(addr, Id::random());
    for _ in 0..8 {
        detector.inconsistent(addr, Some(Id::random()));
    }

    assert_eq!(detector.is_suspicious(&addr), true);
    assert_eq!(detector.is_banned_addr(&addr), false);
}

#[test]
fn test_host_gets_banned_after_threshold() {
    let mut detector = DefaultSuspiciousNodeDetector::new(
        Duration::from_secs(60),
        3,
        Duration::from_secs(60),
    );
    let addr = "127.0.0.1:39003".parse::<SocketAddr>().unwrap();

    detector.inconsistent(addr, Some(Id::random()));
    detector.inconsistent(addr, Some(Id::random()));
    detector.inconsistent(addr, Some(Id::random()));

    assert_eq!(detector.is_banned_addr(&addr), true);
    assert_eq!(detector.banned_size(), 1);
}

#[test]
fn test_reused_id_across_many_addresses_bans_related_hosts() {
    let mut detector = DefaultSuspiciousNodeDetector::new(
        Duration::from_secs(60),
        32,
        Duration::from_secs(60),
    );
    let id = Id::random();
    let addrs = (1..=8)
        .map(|octet| format!("127.0.0.{}:39010", octet).parse::<SocketAddr>().unwrap())
        .collect::<Vec<_>>();

    for addr in &addrs {
        detector.inconsistent(*addr, Some(id));
    }

    assert_eq!(detector.banned_size(), 8);
    for addr in &addrs {
        assert_eq!(detector.is_banned_addr(addr), true);
    }
}

#[test]
fn test_purge_expires_observations_and_bans() {
    let mut detector = DefaultSuspiciousNodeDetector::new(
        Duration::from_millis(1),
        1,
        Duration::from_millis(1),
    );
    let observed_addr = "127.0.0.1:39020".parse::<SocketAddr>().unwrap();
    let banned_addr = "127.0.0.2:39020".parse::<SocketAddr>().unwrap();

    detector.observe(observed_addr, Id::random());
    detector.inconsistent(banned_addr, Some(Id::random()));
    std::thread::sleep(Duration::from_millis(5));
    detector.purge();

    assert_eq!(detector.observed_size(), 0);
    assert_eq!(detector.banned_size(), 0);
}

#[test]
fn test_clear_resets_all_state() {
    let mut detector = DefaultSuspiciousNodeDetector::new(
        Duration::from_secs(60),
        2,
        Duration::from_secs(60),
    );
    let observed_addr = "127.0.0.1:39030".parse::<SocketAddr>().unwrap();
    let banned_addr = "127.0.0.2:39030".parse::<SocketAddr>().unwrap();

    detector.observe(observed_addr, Id::random());
    detector.inconsistent(banned_addr, Some(Id::random()));
    detector.inconsistent(banned_addr, Some(Id::random()));
    detector.clear();

    assert_eq!(detector.observed_size(), 0);
    assert_eq!(detector.banned_size(), 0);
    assert_eq!(detector.is_suspicious(&observed_addr), false);
}
