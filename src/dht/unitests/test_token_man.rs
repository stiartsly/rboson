use std::{
    thread,
    net::SocketAddr,
    time::Duration,
};

use crate::Id;
use crate::dht::{
    token_manager::TokenManager,
};

#[test]
fn test_token() {
    let man = TokenManager::new();

    let nodeid = Id::random();
    let target = Id::random();
    let addr = "192.168.1.123:39001".parse::<SocketAddr>().unwrap();
    thread::sleep(Duration::from_secs(1));

    let token1 = man.generate_token(&nodeid, &addr, &target);
    let token2 = man.generate_token(&nodeid, &addr, &target);
    assert_eq!(token1, token2);
}

#[test]
fn test_token1() {
    let man = TokenManager::new();

    let nodeid = Id::random();
    let target = Id::random();
    let addr = "192.168.1.123:39001".parse::<SocketAddr>().unwrap();
    thread::sleep(Duration::from_secs(1));

    let token = man.generate_token(&nodeid, &addr, &target);
    let result = man.verify_token(token, &nodeid, &addr, &target);
    assert_eq!(result, true);
}
