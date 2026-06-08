use std::{
    net::SocketAddr,
    time::SystemTime,
};

use crate::{
    Id,
    dht::{
    rpc::rpc_target::Reachability,
    routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
        routing_table::RoutingTable,
    },
    }
};

fn make_id(first_byte: u8, last_byte: u8) -> Id {
    let mut bytes = [0u8; Id::BYTES];
    bytes[0] = first_byte;
    bytes[Id::BYTES - 1] = last_byte;
    Id::from_bytes(bytes)
}

fn make_reachable_entry(id: Id, addr: &str) -> KBucketEntry {
    let mut entry = KBucketEntry::new(
        id,
        addr.parse::<SocketAddr>().unwrap()
    );
    entry.on_responded(20);
    entry
}

fn fill_and_split_table() -> (RoutingTable, Id, Id) {
    let local_id = Id::zero();
    let mut rt = RoutingTable::new(local_id);

    for i in 0..KBucket::MAX_ENTRIES {
        let id = make_id(0x00, i as u8 + 1);
        let entry = make_reachable_entry(id, &format!("127.0.0.1:{}", 30000 + i));
        rt.put(entry);
    }

    let high_id = make_id(0x80, 1);
    let high_entry = make_reachable_entry(high_id, "127.0.0.1:31000");
    rt.put(high_entry);

    (rt, make_id(0x00, 1), high_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_and_remove() {
        let local_id = Id::random();
        let mut rt = RoutingTable::new(local_id);

        assert_eq!(rt.size(), 1);
        assert_eq!(rt.is_empty(), false);
        assert_eq!(rt.local_nodeid(), &local_id);
        assert_eq!(rt.is_home_bucket(&rt.buckets().first().unwrap().lock().unwrap().prefix()), true);
        assert_eq!(rt.bucket_entry(Some(&Id::random())).is_none(), true);
        assert_eq!(rt.number_of_entries(), 0);

        let id = make_id(0x00, 1);
        let entry = make_reachable_entry(id, "127.0.0.1:32000");
        rt.put(entry);

        assert_eq!(rt.contains(&id), true);
        assert_eq!(rt.number_of_entries(), 1);
        assert_eq!(rt.bucket_entry(None).is_some(), true);

        let removed = rt.remove(&id);
        assert_eq!(removed.is_some(), true);
        assert_eq!(rt.contains(&id), false);
        assert_eq!(rt.number_of_entries(), 0);
    }

    #[test]
    fn test_bucket_of_and_split() {
        let (rt, low_id, high_id) = fill_and_split_table();
        let buckets = rt.buckets();

        assert_eq!(rt.size(), 2);

        let low_idx = RoutingTable::index_of(&buckets, &low_id);
        let high_idx = RoutingTable::index_of(&buckets, &high_id);

        assert_eq!(low_idx, 0);
        assert_eq!(high_idx, 1);
        assert_eq!(buckets[low_idx].lock().unwrap().prefix().is_prefix_of(&low_id), true);
        assert_eq!(buckets[high_idx].lock().unwrap().prefix().is_prefix_of(&high_id), true);
    }

    #[test]
    fn test_send_timeout_and_responded() {
        let local_id = Id::random();
        let mut rt = RoutingTable::new(local_id);
        let id = make_id(0x00, 2);
        let entry = make_reachable_entry(id, "127.0.0.1:32001");
        rt.put(entry);

        rt.on_request_sent(&id);
        let sent = rt.bucket_entry(Some(&id)).unwrap();
        assert_eq!(sent.last_sent() > &SystemTime::UNIX_EPOCH, true);

        rt.on_timeout(&id);
        assert_eq!(rt.bucket_entry(Some(&id)).unwrap().failed_reqs(), 1);

        rt.on_responded(&id, 55);
        let responsed = rt.bucket_entry(Some(&id)).unwrap();
        assert_eq!(responsed.is_reachable(), true);
        assert_eq!(responsed.failed_reqs(), 0);
        //assert_eq!(responsed.rtt(), 31);
    }
}
