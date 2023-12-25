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

use crate::{
    signature,
    default_configuration as config,
    cryptobox::CryptoBox,
    node_runner::NodeRunner,
    bootstrap_channel::BootstrapChannel,
    future::Command,
};
use crate::{
    create_random_bytes,
    local_addr,
    working_path,
    remove_working_path,
    NodeInfo,
    LookupOption,
    future::FindNodeCmd,
};

static mut PATH1: Option<String> = None;
static mut PATH2: Option<String> = None;

static mut NODE1: Option<Rc<RefCell<NodeRunner>>> = None;
static mut NODE2: Option<Rc<RefCell<NodeRunner>>> = None;

static mut BOOTSTR_CHANNEL:Option<Arc<Mutex<BootstrapChannel>>> = None;
static mut COMMAND_CHANNEL:Option<Arc<Mutex<LinkedList<Command>>>> = None;

fn setup() {
    unsafe {
        let ip_addr = local_addr(true).unwrap();
        let ip_str = ip_addr.to_string();

        PATH1 = Some(working_path("node_runner1/"));
        PATH2 = Some(working_path("node_runner1/"));

        let b1 = config::Builder::new()
            .with_listening_port(32222)
            .with_ipv4(&ip_str)
            .with_storage_path(PATH1.as_ref().unwrap().as_str());
        let cfg1 = b1.build().unwrap();

        let b2 = config::Builder::new()
            .with_listening_port(32224)
            .with_ipv4(&ip_str)
            .with_storage_path(PATH2.as_ref().unwrap().as_str());
        let cfg2 = b2.build().unwrap();

        BOOTSTR_CHANNEL = Some(Arc::new(Mutex::new(BootstrapChannel::new())));
        COMMAND_CHANNEL = Some(Arc::new(Mutex::new(LinkedList::new() as LinkedList<Command>)));

        NODE1 = Some({
            let nr = Rc::new(RefCell::new(NodeRunner::new(
                cfg1.storage_path().to_string(),
                signature::KeyPair::random(),
                Arc::new(Mutex::new(cfg1))
            )));
            nr.borrow_mut().set_field(BOOTSTR_CHANNEL.as_ref().unwrap().clone());
            nr.borrow_mut().set_field(COMMAND_CHANNEL.as_ref().unwrap().clone());
            nr.borrow_mut().set_field(nr.clone());
            nr
        });

        NODE2 = Some({
            let nr = Rc::new(RefCell::new(NodeRunner::new(
                cfg2.storage_path().to_string(),
                signature::KeyPair::random(),
                Arc::new(Mutex::new(cfg2))
            )));
            nr.borrow_mut().set_field(Arc::new(Mutex::new(BootstrapChannel::new())));
            nr.borrow_mut().set_field(Arc::new(Mutex::new(LinkedList::new() as LinkedList<Command>)));
            nr.borrow_mut().set_field(nr.clone());
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

        remove_working_path(PATH1.as_ref().unwrap().as_str());
        remove_working_path(PATH2.as_ref().unwrap().as_str());

        PATH1 = None;
        PATH2 = None;
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
        assert_eq!(plain.len() + CryptoBox::MAC_BYTES, cipher.len());

        let result = node2.borrow_mut().decrypt_into(&node1.borrow().id(), &cipher);
        assert_eq!(result.is_ok(), true);

        let decrypted = result.ok().unwrap();
        assert_eq!(cipher.len() - CryptoBox::MAC_BYTES, decrypted.len());
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

        let channel = BOOTSTR_CHANNEL.as_ref().unwrap().clone();
        let node2id = node2.borrow().id().deref().clone();
        let ni2 = NodeInfo::new(
            node2id.clone(),
            SocketAddr::new(local_addr(true).unwrap(), 32224)
        );
        channel.lock().unwrap().push(&ni2);

        let opt = LookupOption::Conservative;
        let arc = Arc::new(Mutex::new(FindNodeCmd::new(&node2id, &opt)));
        let cmd = Command::FindNode(arc.clone());
        let channel = COMMAND_CHANNEL.as_ref().unwrap();
        channel.lock().unwrap().push_back(cmd.clone());

        while !cmd.is_completed() {
            thread::sleep(Duration::from_secs(1));
            println!(">>>");
        }
       // let result = arc.lock().unwrap().result();
        println!("found");
    }
    teardown()
}
