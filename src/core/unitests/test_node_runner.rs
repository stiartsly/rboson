use std::net::SocketAddr;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::collections::LinkedList;
use core::ops::Deref;
use serial_test::serial;
use once_cell::sync::Lazy;

use super::{
    create_random_bytes,
    working_path,
    remove_working_path
};

use crate::{
    local_addr,
    signature,
    cryptobox,
    Network,
    NodeInfo,
    CryptoBox,
    LookupOption,
    configuration as config,
    JointResult,
    core::crypto_cache::CryptoCache,
};

use crate::core::{
    node_runner::NodeRunner,
    bootstrap_channel::BootstrapChannel,
    future::{Command, FindNodeCmd}
};

static PATH1: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(working_path("node_runner1/")));
static PATH2: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(working_path("node_runner2/")));

static mut NODE1: Option<Rc<RefCell<NodeRunner>>> = None;
static mut NODE2: Option<Rc<RefCell<NodeRunner>>> = None;

static BOOTSTR_CHANNEL: Lazy<Arc<Mutex<BootstrapChannel>>> = Lazy::new(|| {
    Arc::new(Mutex::new(BootstrapChannel::new()))
});

static COMMAND_CHANNEL: Lazy<Arc<Mutex<LinkedList<Command>>>> = Lazy::new(|| {
    Arc::new(Mutex::new(LinkedList::new() as LinkedList<Command>))
});

static KEYPAIR1: Lazy<signature::KeyPair> = Lazy::new(|| {
    signature::KeyPair::random()
});
static CRYPTO_CACHE1:Lazy<Arc<Mutex<CryptoCache>>> = Lazy::new(||{
    Arc::new(Mutex::new(CryptoCache::new(cryptobox::KeyPair::from(&KEYPAIR1.clone()))))
});

static KEYPAIR2: Lazy<signature::KeyPair> = Lazy::new(|| {
    signature::KeyPair::random()
});
static CRYPTO_CACHE2:Lazy<Arc<Mutex<CryptoCache>>> = Lazy::new(||{
    Arc::new(Mutex::new(CryptoCache::new(cryptobox::KeyPair::from(&KEYPAIR2.clone()))))
});

fn setup() {
    unsafe {
        let ip_addr = local_addr(true).unwrap();
        let ip_str = ip_addr.to_string();
        let cfg1 = config::Builder::new()
            .with_listening_port(32222)
            .with_ipv4(&ip_str)
            .with_storage_path(&PATH1.lock().unwrap())
            .build()
            .unwrap();

        let cfg2 = config::Builder::new()
            .with_listening_port(32224)
            .with_ipv4(&ip_str)
            .with_storage_path(&PATH2.lock().unwrap())
            .build()
            .unwrap();

        let mut addrs1: JointResult<SocketAddr> = JointResult::new();
        if let Some(addr4) = cfg1.addr4() {
            addrs1.set_value(Network::IPv4, addr4.clone());
        }
        if let Some(addr6) = cfg1.addr6() {
            addrs1.set_value(Network::IPv6, addr6.clone());
        }

        let mut addrs2: JointResult<SocketAddr> = JointResult::new();
        if let Some(addr4) = cfg2.addr4() {
            addrs2.set_value(Network::IPv4, addr4.clone());
        }
        if let Some(addr6) = cfg2.addr6() {
            addrs2.set_value(Network::IPv6, addr6.clone());
        }

        NODE1 = Some({
            let nr = Rc::new(RefCell::new(NodeRunner::new(
                cfg1.storage_path().to_string(),
                KEYPAIR1.clone(),
                addrs1,
                COMMAND_CHANNEL.clone(),
                BOOTSTR_CHANNEL.clone(),
                CRYPTO_CACHE1.clone()
            )));
            nr.borrow_mut().set_cloned(nr.clone());
            nr
        });

        NODE2 = Some({
            let nr = Rc::new(RefCell::new(NodeRunner::new(
                cfg2.storage_path().to_string(),
                KEYPAIR2.clone(),
                addrs2,
                COMMAND_CHANNEL.clone(),
                BOOTSTR_CHANNEL.clone(),
                CRYPTO_CACHE2.clone()
            )));
            nr.borrow_mut().set_cloned(nr.clone());
            nr
        });
        let _ = NODE1.as_mut().unwrap().borrow_mut().start();
        let _ = NODE2.as_mut().unwrap().borrow_mut().start();
    }
}

fn teardown() {
    unsafe {
        NODE1.as_mut().unwrap().borrow_mut().stop();
        NODE2.as_mut().unwrap().borrow_mut().stop();

        NODE1 = None;
        NODE2 = None;

        remove_working_path(&PATH1.lock().unwrap());
        remove_working_path(&PATH2.lock().unwrap());
    }
}

/*
 Testcases for critical methods:
 - encrypt_into(..)
 - decrypt_into(..)
 - find_node(..)
 */
#[test]
#[serial]
fn test_encrypt_into() {
    setup();
    unsafe {
        let node1 = NODE1.as_mut().unwrap();
        let node2 = NODE2.as_mut().unwrap();

        let plain = create_random_bytes(32);
        let result = node1.borrow_mut().encrypt_into(&node2.borrow().id(), &plain);
        assert_eq!(result.is_ok(), true);
        let cipher = result.ok().unwrap();
        assert_eq!(plain.len() + CryptoBox::MAC_BYTES + cryptobox::Nonce::BYTES, cipher.len());

        let result = node2.borrow_mut().decrypt_into(&node1.borrow().id(), &cipher);
        assert_eq!(result.is_ok(), true);

        let decrypted = result.ok().unwrap();
        assert_eq!(cipher.len() - CryptoBox::MAC_BYTES - cryptobox::Nonce::BYTES, decrypted.len());
        assert_eq!(plain.len(), decrypted.len());
        assert_eq!(plain, decrypted);
    }
    teardown()
}

//#[test]
#[allow(dead_code)]
fn test_find_node() {
    setup();
    unsafe {
        //fn find_node(&self, cmd: Arc<Mutex<FindNodeCmd>>)
        //let node1 = NODE1.as_ref().unwrap();
        let node2 = NODE2.as_ref().unwrap().clone();

        let channel = BOOTSTR_CHANNEL.clone();
        let node2id = node2.borrow().id().deref().clone();
        let ni2 = NodeInfo::new(
            node2id.clone(),
            SocketAddr::new(local_addr(true).unwrap(), 32224)
        );
        channel.lock().unwrap().push(&ni2);

        let opt = LookupOption::Conservative;
        let arc = Arc::new(Mutex::new(FindNodeCmd::new(&node2id, &opt)));
        let cmd = Command::FindNode(arc.clone());
        COMMAND_CHANNEL.lock().unwrap().push_back(cmd.clone());

        while !cmd.is_completed() {
            thread::sleep(Duration::from_secs(1));
            println!(">>>");
        }
       // let result = arc.lock().unwrap().result();
        println!("found");
    }
    teardown()
}
