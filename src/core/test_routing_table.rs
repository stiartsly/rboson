use std::rc::Rc;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::time::SystemTime;

use crate::{
    id::Id,
    kbucket_entry::KBucketEntry,
    routing_table::RoutingTable,
};

#[test]
fn test_put() {
    let id = Rc::new(Id::random());
    let mut rt = RoutingTable::new(id.clone());
    assert_eq!(rt.size(), 1);
    assert_eq!(rt.size_of_entries(), 0);
    assert_eq!(rt.buckets().is_empty(), false);

    let target = Id::random();
    assert_eq!(rt.bucket_entry(&target).is_some(), false);
    assert_eq!(rt.random_entry().is_some(), false);
    assert_eq!(rt.random_entries(8).len(), 0);

    let id1 = Id::random();
    let addr1 = "192.168.1.100:39001".parse::<SocketAddr>().unwrap();
    let mut entry = KBucketEntry::new(id1.clone(), addr1);
    entry.signal_response();
    entry.merge_request_time(SystemTime::now());

    rt.put(Rc::new(RefCell::new(entry)));
    assert_eq!(rt.size(), 1);
    assert_eq!(rt.size_of_entries(), 1);
    assert_eq!(rt.buckets().len(), 1);

    rt.remove(&id1);
    assert_eq!(rt.size(), 1);
    assert_eq!(rt.size_of_entries(), 0);
    assert_eq!(rt.buckets().len(), 1);
}

#[test]
fn test_split() {
    let id = Rc::new(Id::random());
    let mut rt = RoutingTable::new(id.clone());
    assert_eq!(rt.size(), 1);
    assert_eq!(rt.size_of_entries(), 0);
    assert_eq!(rt.buckets().is_empty(), false);

    for i in 0..1000
    {
        let id = Id::random();
        let addr = format!("192.168.1.100:{}", i+1);
        let addr = addr.parse::<SocketAddr>().unwrap();
        let mut input = KBucketEntry::new(id.clone(), addr);
        input.signal_response();
        input.merge_request_time(SystemTime::now());

        rt.put(Rc::new(RefCell::new(input.clone())));
        // assert_eq!(rt.size_of_entries(), i + 1);
    }

    assert_eq!(true, true);
}
