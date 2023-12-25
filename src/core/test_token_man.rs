use std::{
    thread,
    net::SocketAddr,
    time::Duration,
};

use crate::{
    Id,
    token::TokenManager,
};

#[test]
fn test_token() {
    let mut man = TokenManager::new();

    let nodeid = Id::random();
    let target = Id::random();
    let addr = "192.168.1.123:39001".parse::<SocketAddr>().unwrap();
    thread::sleep(Duration::from_secs(1));

    let token = man.generate_token(&nodeid, &addr, &target);
    let result = man.verify_token(token, &nodeid, &addr, &target);

    // assert_eq!(token > 0, true); TODO:
    assert_eq!(result, true);
}
