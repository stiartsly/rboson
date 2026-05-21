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

fn build_split_table() -> Arc<Mutex<RoutingTable>> {
    let mut rt = RoutingTable::new(Id::zero());
    for i in 0..KBucket::MAX_ENTRIES {
        rt.put(make_reachable_entry(make_id(0x00, i as u8 + 1), 33000 + i as u16));
    }
    rt.put(make_reachable_entry(make_id(0x80, 1), 34000));
    Arc::new(Mutex::new(rt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_accessors() {
        let table = build_split_table();
        let target = make_id(0x80, 1);
        let mut closest = KClosestNodes::new(table.clone(), target, 4);

        assert_eq!(closest.target(), &make_id(0x80, 1));
        assert_eq!(closest.size(), 0);
        assert_eq!(closest.is_full(), false);
        assert_eq!(closest.is_complete(), false);

        closest.fill();

        assert_eq!(closest.size() <= 4, true);
        assert_eq!(closest.entries().is_empty(), false);
        assert_eq!(closest.nodes().len(), closest.entries().len());
    }

    #[test]
    fn test_fill_orders_by_distance() {
        let table = build_split_table();
        let target = make_id(0x00, 2);
        let mut closest = KClosestNodes::new(table.clone(), target, 5);
        closest.fill();

        let local_id = *table.lock().unwrap().local_id();
        assert!(!closest.entries().iter().any(|entry| entry.id() == &local_id));

        let mut previous = None;
        for entry in closest.entries() {
            if let Some(prev) = previous {
                assert_ne!(target.three_way_compare(prev, entry.id()), std::cmp::Ordering::Greater);
            }
            previous = Some(entry.id());
        }
    }

    #[test]
    fn test_filter_is_chainable() {
        let table = build_split_table();
        let target = make_id(0x00, 3);
        let mut closest = KClosestNodes::new(table.clone(), target, 8);

        closest
            .filter(|entry| entry.socket_addr().port() % 2 == 0)
            .fill();

        assert!(closest.entries().iter().all(|entry| entry.socket_addr().port() % 2 == 0));
    }

    #[test]
    fn test_set_filter() {
        let table = build_split_table();
        let target = make_id(0x80, 1);
        let mut closest = KClosestNodes::new(table.clone(), target, 8);

        closest.set_filter(|entry| entry.id().as_bytes()[0] == 0x80);
        closest.fill();

        assert_eq!(closest.entries().iter().all(|entry| entry.id().as_bytes()[0] == 0x80), true);
        assert_eq!(closest.size() >= 1, true);
    }

    #[test]
    fn test_fill_with_empty_table() {
        let table = Arc::new(Mutex::new(RoutingTable::new(Id::zero())));
        let mut closest = KClosestNodes::new(table, make_id(0x40, 1), 4);
        closest.fill();

        assert_eq!(closest.entries().is_empty(), true);
        assert_eq!(closest.nodes().is_empty(), true);
    }
}
