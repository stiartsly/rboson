use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use crate::Id;
use crate::dht::rpc::Reachability;
use crate::dht::routing::{Prefix, KBucket, KBucketEntry};

#[cfg(test)]
mod tests {
    use super::*;

    fn make_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    fn make_entry(id: Id, port: u16) -> KBucketEntry {
        KBucketEntry::new(id, make_addr(port))
    }

    #[ignore]
    #[test]
    fn test_rbtree() {
        use rbtree::RBTree;
        let mut tree = RBTree::<i32, &str>::new();
        tree.insert(3, "three");
        tree.insert(1, "one");
        tree.insert(4, "four");
        tree.insert(2, "two");

        let mut iter = tree.iter();
        assert_eq!(iter.next(), Some((&1, &"one")));
        assert_eq!(iter.next(), Some((&2, &"two")));
        assert_eq!(iter.next(), Some((&3, &"three")));
        assert_eq!(iter.next(), Some((&4, &"four")));
        assert_eq!(iter.next(), None);

        println!("RBTree keys: {}", tree.keys().collect::<Vec<_>>().iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", "));

        use std::time::{SystemTime, Duration};
        let mut tree = RBTree::<SystemTime, &str>::new();

        let first_tm = SystemTime::now();
        let scond_tm = first_tm + Duration::from_secs(50);
        let third_tm = first_tm + Duration::from_secs(100);

        tree.insert(third_tm, "third");
        tree.insert(scond_tm, "second");
        tree.insert(first_tm, "first");

        let mut iter = tree.iter();
        assert_eq!(iter.next(), Some((&first_tm, &"first")));
        assert_eq!(iter.next(), Some((&scond_tm, &"second")));
        assert_eq!(iter.next(), Some((&third_tm, &"third")));
        assert_eq!(iter.next(), None);

        println!("RBTree entries: {}", tree.iter().map(|(k, v)| format!("{}: {}", k.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), v)).collect::<Vec<_>>().join(", "));
    }

    #[test]
    fn test_new() {
        let prefix = Prefix::new();
        let bucket = KBucket::new(prefix, false);
        assert_eq!(bucket.prefix(), &prefix);
        assert_eq!(bucket.size(), 0);

        assert!(!bucket.is_home_bucket());
        assert!(bucket.is_empty());
        assert!(!bucket.is_full());
        assert!(bucket.entries().is_empty());
    }

    #[test]
    fn test_home_bucket() {
        let prefix = Prefix::new();
        let bucket = KBucket::home_bucket(prefix);
        assert_eq!(bucket.prefix(), &prefix);
        assert!(bucket.is_home_bucket());
        assert!(bucket.is_empty());
        assert!(!bucket.is_full());
        assert!(bucket.entries().is_empty());
    }

    #[test]
    fn test_put() {
        let prefix = Prefix::new();
        let mut bucket = KBucket::new(prefix, false);
        let id = Id::random();
        let mut entry = make_entry(id.clone(), 1234);
        entry.set_reachable(true);

        bucket.put(entry);
        assert_eq!(bucket.size(), 1);
        assert!(bucket.contains(&id));

        let entry = bucket.entry(Some(&id));
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.id(), &id);
        assert!(entry.is_reachable());

        /*let entry = bucket.remove(&id);
        assert!(entry.is_some());
        assert_eq!(bucket.size(), 0);
        */

        let prefix = Prefix::new();
        let mut bucket = KBucket::new(prefix, false);
        let id = Id::random();
        let mut entry1 = make_entry(id.clone(), 1234);
        entry1.set_reachable(true);
        let mut entry2 = make_entry(id.clone(), 1234);
        entry2.set_reachable(true);

        bucket.put(entry1);
        bucket.put(entry2);
        assert_eq!(bucket.size(), 1);
    }

    #[test]
    fn test_is_full() {
        let prefix = Prefix::new();
        let mut bucket = KBucket::new(prefix, false);

        for i in 0..KBucket::MAX_ENTRIES {
            let id = Id::random();
            let mut entry = make_entry(id, 1000 + i as u16);
            entry.set_reachable(true);
            bucket.put(entry);
        }

        assert_eq!(bucket.size(), KBucket::MAX_ENTRIES);
        assert!(bucket.is_full());
    }

    #[test]
    fn test_retrieval() {
        let prefix = Prefix::new();
        let mut bucket = KBucket::new(prefix, false);
        let id = Id::random();
        let mut entry = make_entry(id.clone(), 1234);
        entry.set_reachable(true);

        bucket.put(entry);

        // Test retrieval by ID
        let retrieved = bucket.entry(Some(&id));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), &id);

        // Test random retrieval
        let random_retrieved = bucket.entry(None);
        assert!(random_retrieved.is_some());
        assert_eq!(random_retrieved.unwrap().id(), &id);
    }

    #[test]
    fn test_on_timeout_and_responded() {
        let prefix = Prefix::new();
        let mut bucket = KBucket::new(prefix, false);
        let id = Id::random();
        let mut entry = make_entry(id.clone(), 1234);
        entry.set_reachable(true);

        bucket.put(entry);

        // Test on_timeout
        bucket.on_timeout(&id);
        let entry_after_timeout = bucket.entry(Some(&id)).unwrap();
        // Since we can't easily check failed_reqs without #[cfg(test)] in KBucketEntry (which it has for some fields)
        // We'll just ensure it doesn't crash and maybe check if we can see the effect.
        // Actually KBucketEntry has failed_reqs() as #[cfg(test)].
        assert_eq!(entry_after_timeout.failed_reqs(), 1);

        // Test on_responded
        bucket.on_responded(&id, 100);
        let entry_after_responded = bucket.entry(Some(&id)).unwrap();
        assert_eq!(entry_after_responded.failed_reqs(), 0);
        assert!(entry_after_responded.is_reachable());
    }
}
