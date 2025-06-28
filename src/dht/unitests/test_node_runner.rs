use std::net::SocketAddr;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::LinkedList;
use serial_test::serial;
use once_cell::sync::Lazy;

use super::{
    working_path,
    remove_working_path
};

use crate::{
    random_bytes as create_random_bytes,
    local_addr,
    signature,
    cryptobox,
    Network,
    CryptoBox,
    configuration as config,
    JointResult
};

use crate::dht::{
    crypto_cache::CryptoCache,
    node_runner::NodeRunner,
    bootstrap_channel::BootstrapChannel,
    future::{Command}
};

static PATH1: Lazy<String> = Lazy::new(|| working_path("node_runner1/"));
static PATH2: Lazy<String> = Lazy::new(|| working_path("node_runner2/"));

static KEYPAIR1: Lazy<signature::KeyPair> = Lazy::new(|| {
    signature::KeyPair::random()
});

static KEYPAIR2: Lazy<signature::KeyPair> = Lazy::new(|| {
    signature::KeyPair::random()
});

fn create_node(port: u16, path: &str, keypair: &signature::KeyPair) -> Rc<RefCell<NodeRunner>> {
    let ip_addr = local_addr(true).unwrap();
    let ip_str = ip_addr.to_string();
    let cfg = config::Builder::new()
        .with_listening_port(port)
        .with_ipv4(&ip_str)
        .with_storage_path(path)
        .build()
        .unwrap();

    let mut addrs1: JointResult<SocketAddr> = JointResult::new();
    if let Some(addr4) = cfg.addr4() {
        addrs1.set_value(Network::IPv4, addr4.clone());
    }
    if let Some(addr6) = cfg.addr6() {
        addrs1.set_value(Network::IPv6, addr6.clone());
    }

    let nr = Rc::new(RefCell::new(NodeRunner::new(
        cfg.storage_path().to_string(),
        keypair.clone(),
        addrs1,
        Arc::new(Mutex::new(LinkedList::new() as LinkedList<Command>)),
        Arc::new(Mutex::new(BootstrapChannel::new())),
        Arc::new(Mutex::new(CryptoCache::new(cryptobox::KeyPair::from(keypair))))
    )));
    nr.borrow_mut().set_cloned(nr.clone());
    nr
}

/*
 Testcases for critical methods:
 - encrypt_into(..)
 - decrypt_into(..)
 - find_node(..)
 */
#[test]
#[serial]
#[ignore]
fn test_encrypt_into() {
    let node_runner1 = create_node(32222, &PATH1, &KEYPAIR1);
    _ = node_runner1.borrow_mut().start();

    let node_runner2 = create_node(32224, &PATH2, &KEYPAIR2);
    _ = node_runner2.borrow_mut().start();

    let plain = create_random_bytes(32);
    let result = node_runner1.borrow_mut().encrypt_into(&node_runner2.borrow().id(), &plain);
    assert_eq!(result.is_ok(), true);
    let cipher = result.ok().unwrap();
    assert_eq!(plain.len() + CryptoBox::MAC_BYTES + cryptobox::Nonce::BYTES, cipher.len());

    let result = node_runner2.borrow_mut().decrypt_into(&node_runner1.borrow().id(), &cipher);
    assert_eq!(result.is_ok(), true);

    let decrypted = result.ok().unwrap();
    assert_eq!(cipher.len() - CryptoBox::MAC_BYTES - cryptobox::Nonce::BYTES, decrypted.len());
    assert_eq!(plain.len(), decrypted.len());
    assert_eq!(plain, decrypted);

    node_runner1.borrow_mut().stop();
    node_runner2.borrow_mut().stop();

    remove_working_path(&PATH1);
    remove_working_path(&PATH2);
}
