use std::{
    net::SocketAddr,
};

use crate::{
    Id,
    NodeInfo,
    dht::rpc::rpc_target::NodeInfoLike,
    dht::routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
        kclosest_nodes::KClosestNodes,
        routing_table::RoutingTable,
    },
};

fn make_id(first_byte: u8, last_byte: u8) -> Id {
    let mut bytes = [0u8; Id::BYTES];
    bytes[0] = first_byte;
    bytes[Id::BYTES - 1] = last_byte;
    Id::from_bytes(bytes)
}

fn make_kentry(id: Id, port: u16) -> KBucketEntry {
    let mut entry = KBucketEntry::new(
        id,
        format!("127.0.0.1:{port}").parse::<SocketAddr>().unwrap(),
    );
    entry.on_responded(20);
    entry
}

fn make_rt(local_id: Id) -> RoutingTable {
    RoutingTable::new(local_id)
}

fn make_split_rt() -> RoutingTable {
    let mut rt = make_rt(Id::zero());
    for i in 0..KBucket::MAX_ENTRIES {
        rt.put(make_kentry(make_id(0x00, i as u8 + 1), 33000 + i as u16));
    }
    rt.put(make_kentry(make_id(0x80, 1), 34000));
    rt
}

fn build_rt_with_local_entry() -> RoutingTable {
    let local_id = make_id(0x00, 1);
    let mut rt = make_rt(local_id);
    rt.put(make_kentry(local_id, 35000));
    rt.put(make_kentry(make_id(0x00, 2), 35001));
    rt.put(make_kentry(make_id(0x80, 1), 35002));
    rt
}

fn make_small_rt() -> RoutingTable {
    let mut rt = make_rt(Id::zero());
    rt.put(make_kentry(make_id(0x00, 1), 36000));
    rt.put(make_kentry(make_id(0x80, 1), 36001));
    rt
}

fn make_closest(rt: &RoutingTable, target: Id, capacity: usize) -> KClosestNodes {
    KClosestNodes::new(rt, target, capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill() {
        let target = make_id(0x80, 1);
        let capacity = 4;
        let rt = make_split_rt();
        let mut closest = make_closest(&rt, target.clone(), capacity);

        assert_eq!(closest.target(), &target);
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
        let mut closest = make_closest(&rt, target, 5);
        closest.fill();

        let local_id = rt.local_nodeid().clone();
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
    fn test_set_filter() {
        let rt = make_split_rt();
        let target = make_id(0x00, 3);
        let mut closest = make_closest(&rt, target, 8);

        closest.set_filter(|entry| entry.socket_addr().port() % 2 == 0);
        closest.fill();

        assert!(!closest.is_full());

        assert!(closest.entries().iter().all(
            |entry| entry.socket_addr().port() % 2 == 0
        ));

        let rt = make_split_rt();
        let target = make_id(0x80, 1);
        let mut closest = make_closest(&rt, target, 8);

        closest.set_filter(|entry| entry.id().as_bytes()[0] == 0x80);
        closest.fill();

        assert!(closest.entries().iter().all(|entry| entry.id().as_bytes()[0] == 0x80));
        assert!(closest.size() >= 1);
    }

    #[test]
    fn test_set_filter_excludes_local_nodeid() {
        let rt = build_rt_with_local_entry();
        let local_id = rt.local_nodeid().clone();
        let mut closest = make_closest(&rt, make_id(0x00, 1), 4);

        closest.set_filter(|_| true);
        closest.fill();

        assert!(!closest.entries().iter().any(|entry| entry.id() == &local_id));
        assert!(closest.entries().iter().all(|entry| entry.id() != &local_id));
    }

    #[test]
    fn test_fill_with_empty_rt(){
        let rt = make_rt(Id::zero());
        let mut closest = make_closest(&rt, make_id(0x40, 1), 4);
        closest.fill();

        assert!(closest.entries().is_empty());
    }

    #[test]
    fn test_fill_with_zero_capacity() {
        let rt = make_split_rt();
        let mut closest = make_closest(&rt, make_id(0x40, 1), 0);

        closest.fill();

        assert_eq!(closest.size(), 0);
        assert!(closest.entries().is_empty());
        assert!(closest.is_full());
    }

    #[test]
    fn test_fill_twice() {
        let rt = make_small_rt();
        let mut closest = make_closest(&rt, make_id(0x40, 1), 4);

        closest.fill();
        assert_eq!(closest.size(), 2);

        closest.fill();
        assert_eq!(closest.size(), 4);
    }

    #[test]
    fn test_into_vec_kbucket_entry() {
        let rt = make_split_rt();
        let target = make_id(0x00, 4);
        let mut closest = make_closest(&rt, target.clone(), 3);

        closest.fill();

        let expected_ids: Vec<Id> = closest
            .entries()
            .iter()
            .map(|entry| entry.id().clone())
            .collect();
        let entries: Vec<KBucketEntry> = closest.into();

        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries.iter().map(|entry| entry.id().clone()).collect::<Vec<_>>(),
            expected_ids
        );
        assert!(entries.iter().all(|entry| entry.id() != rt.local_nodeid()));
    }

    #[test]
    fn test_into_vec_node_info() {
        let rt = make_split_rt();
        let mut closest = make_closest(&rt, make_id(0x80, 1), 4);

        closest.fill();

        let expected: Vec<NodeInfo> = closest
            .entries()
            .iter()
            .map(|entry| entry.ni())
            .collect();
        let nodes: Vec<NodeInfo> = closest.into();

        assert_eq!(nodes, expected);
    }
}
