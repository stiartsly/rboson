use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use crate::{
    Id,
    dht::routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
        kclosest_nodes::KClosestNodes,
        routing_table::RoutingTable,
    }
};

fn make_id(first_byte: u8, last_byte: u8) -> Id {
    let mut bytes = [0u8; Id::BYTES];
    bytes[0] = first_byte;
    bytes[Id::BYTES - 1] = last_byte;
    Id::from_bytes(bytes)
}

fn make_reachable_entry(id: Id, port: u16) -> KBucketEntry {
    let mut entry = KBucketEntry::new(
        id,
        format!("127.0.0.1:{port}").parse::<SocketAddr>().unwrap(),
    );
    entry.on_responded(20);
    entry
}

fn make_split_rt() -> Arc<Mutex<RoutingTable>> {
    let mut rt = RoutingTable::new(Id::zero());
    for i in 0..KBucket::MAX_ENTRIES {
        rt.put(make_reachable_entry(make_id(0x00, i as u8 + 1), 33000 + i as u16));
    }
    rt.put(make_reachable_entry(make_id(0x80, 1), 34000));
    Arc::new(Mutex::new(rt))
}

fn build_table_with_local_entry() -> Arc<Mutex<RoutingTable>> {
    let local_id = make_id(0x00, 1);
    let mut rt = RoutingTable::new(local_id);
    rt.put(make_reachable_entry(local_id, 35000));
    rt.put(make_reachable_entry(make_id(0x00, 2), 35001));
    rt.put(make_reachable_entry(make_id(0x80, 1), 35002));
    Arc::new(Mutex::new(rt))
}

fn make_small_rt() -> Arc<Mutex<RoutingTable>> {
    let mut rt = RoutingTable::new(Id::zero());
    rt.put(make_reachable_entry(make_id(0x00, 1), 36000));
    rt.put(make_reachable_entry(make_id(0x80, 1), 36001));
    Arc::new(Mutex::new(rt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill() {
        let rt = make_split_rt();
        let target = make_id(0x80, 1);
        let capacity = 4;
        let mut closest = KClosestNodes::new(rt, target, capacity);

        assert_eq!(closest.target(), &make_id(0x80, 1));
        assert_eq!(closest.size(), 0);
        assert!(!closest.is_full());

        closest.fill();

        assert_eq!(closest.size(), capacity);
        assert!(closest.is_full());
        assert!(closest.entries().len() > 0);

        let entries: Vec<KBucketEntry> = closest.into();
        assert!(entries.len() <= capacity);
        assert!(entries.len() > 0);
    }

    #[test]
    fn test_fill_orders_by_distance() {
        let rt = make_split_rt();
        let target = make_id(0x00, 2);
        let mut closest = KClosestNodes::new(rt.clone(), target, 5);
        closest.fill();

        let local_id = *rt.lock().unwrap().local_nodeid();
        assert!(!closest.entries()
            .iter()
            .any(|entry| entry.id() == &local_id));

        let mut previous = None;
        for entry in closest.entries() {
            if let Some(prev) = previous {
                assert_ne!(target.three_way_compare(prev, entry.id()), std::cmp::Ordering::Greater);
            }
            previous = Some(entry.id());
        }
    }

    #[test]
    fn test_set_filter1() {
        let rt = make_split_rt();
        let target = make_id(0x00, 3);
        let mut closest = KClosestNodes::new(rt.clone(), target, 8);

        closest.set_filter(|entry| entry.socket_addr().port() % 2 == 0);
        closest.fill();

        assert!(!closest.is_full());

        assert!(closest.entries().iter().all(
            |entry| entry.socket_addr().port() % 2 == 0
        ));
    }

    #[test]
    fn test_set_filter2() {
        let rt = make_split_rt();
        let target = make_id(0x80, 1);
        let mut closest = KClosestNodes::new(rt.clone(), target, 8);

        closest.set_filter(|entry| entry.id().as_bytes()[0] == 0x80);
        closest.fill();

        assert!(closest.entries().iter().all(|entry| entry.id().as_bytes()[0] == 0x80));
        assert!(closest.size() >= 1);
    }

    #[test]
    fn test_set_filter_excludes_local_nodeid() {
        let rt = build_table_with_local_entry();
        let local_id = *rt.lock().unwrap().local_nodeid();
        let mut closest = KClosestNodes::new(rt, make_id(0x00, 1), 4);

        closest.set_filter(|_| true);
        closest.fill();

        assert!(!closest.entries().iter().any(|entry| entry.id() == &local_id));
        assert!(closest.entries().iter().all(|entry| entry.id() != &local_id));
    }

    #[test]
    fn test_fill_with_empty_rt(){
        let rt = Arc::new(Mutex::new(RoutingTable::new(Id::zero())));
        let mut closest = KClosestNodes::new(rt, make_id(0x40, 1), 4);
        closest.fill();

        assert!(closest.entries().is_empty());
    }

    #[test]
    fn test_fill_with_zero_capacity() {
        let rt = make_split_rt();
        let mut closest = KClosestNodes::new(rt, make_id(0x40, 1), 0);

        closest.fill();

        assert_eq!(closest.size(), 0);
        assert!(closest.entries().is_empty());
        assert!(closest.is_full());
    }

    #[test]
    fn test_fill_twice() {
        let rt = make_small_rt();
        let mut closest = KClosestNodes::new(rt, make_id(0x40, 1), 4);

        closest.fill();
        assert_eq!(closest.size(), 2);

        closest.fill();
        assert_eq!(closest.size(), 4);
    }
}
