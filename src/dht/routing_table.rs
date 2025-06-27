use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Write};
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use rbtree::RBTree;
use ciborium::value::Value;
use libsodium_sys::randombytes_uniform;
use log::{info, warn};

use crate::{
    as_millis,
    Id,
    Prefix,
    core::node_info::Reachable,
};

use crate::dht::{
    constants,
    cbor,
    dht::DHT,
    kbucket::KBucket,
    kbucket_entry::KBucketEntry,
    task::{
        ping_refresh::PingOption,
        ping_refresh::PingRefreshTask,
        task::Task
    }
};

pub(crate) struct RoutingTable {
    nodeid: Rc<Id>, // Node Id of current DHT Node.
    dht: Option<Rc<RefCell<DHT>>>,
    buckets: RBTree<Rc<Prefix>, Rc<RefCell<KBucket>>>,

    time_of_last_ping_check: SystemTime,
    maintenance_tasks: HashMap<Rc<Prefix>, Rc<RefCell<Box<dyn Task>>>>,
}

impl RoutingTable {
    pub(crate) fn new(nodeid: Rc<Id>) -> Self {
        let buckets = {
            let prefix = Rc::new(Prefix::new());
            let bucket = KBucket::with_homebucket(prefix.clone());
            let mut bs = RBTree::new();
            bs.insert(prefix, Rc::new(RefCell::new(bucket)));
            bs
        };

        Self {
            nodeid,
            dht: None,
            buckets,

            time_of_last_ping_check: SystemTime::UNIX_EPOCH,
            maintenance_tasks: HashMap::new(),
        }
    }

    pub(crate) fn set_dht(&mut self, dht: Rc<RefCell<DHT>>) {
        self.dht = Some(dht)
    }

    fn dht(&self) -> Rc<RefCell<DHT>> {
        assert!(self.dht.is_some());
        self.dht.as_ref().unwrap().clone()
    }

    pub(crate) fn size(&self) -> usize {
        self.buckets.len()
    }

    pub(crate) fn size_of_entries(&self) -> usize {
        let mut total = 0;
        self.buckets.iter().for_each(|(_,v)| {
            total += v.borrow().size()
        });
        total
    }

    pub(crate) fn buckets(&self) -> &RBTree<Rc<Prefix>, Rc<RefCell<KBucket>>> {
        &self.buckets
    }

    pub(crate) fn bucket(&self, target: &Id) -> Rc<RefCell<KBucket>> {
        self.buckets.iter()
            .find(|(k,_)| k.is_prefix_of(target))
            .map(|(_,v)| v.clone())
            .unwrap()
    }

    fn pop_bucket(&mut self, target: &Id) -> Rc<RefCell<KBucket>> {
        let key = self.buckets.iter()
            .find(|(k,_v)| k.is_prefix_of(target))
            .map(|(k, _)| k)
            .unwrap()
            .clone();

        self.buckets.remove(&key).unwrap()
    }

    pub(crate) fn bucket_entry(&self, target: &Id) -> Option<Rc<RefCell<KBucketEntry>>> {
        self.bucket(target).borrow().entry(target)
    }

    pub(crate) fn random_entry(&self) -> Option<Rc<RefCell<KBucketEntry>>> {
        let key = {
            let keys = self.buckets.keys().collect::<Vec<_>>();
            let rand = unsafe {
                randombytes_uniform(keys.len() as u32)
            } as usize;
            keys[rand]
        };

        self.buckets.get(key).unwrap().borrow()
            .random_entry()
    }

    pub(crate) fn random_entries(&self, expected: usize) -> Vec<Rc<RefCell<KBucketEntry>>> {
        let mut total = 0;
        self.buckets.iter().for_each(|(_,v)| {
            total += v.borrow().size();
        });

        if total < expected {
            let mut entries = Vec::new();
            self.buckets.iter().for_each(|(_, v)| {
                entries.append(&mut v.borrow().entries())
            });
            return entries;
        }

        // TODO:
        Vec::new()
    }

    pub(crate) fn put(&mut self, entry: Rc<RefCell<KBucketEntry>>) {
        self._put(entry)
    }

    pub(crate) fn remove(&mut self, id: &Id) {
        self._remove(id)
    }

    pub(crate) fn on_timeout(&mut self, id: &Id) {
        self._on_timeout(id)
    }

    pub(crate) fn on_send(&mut self, id: &Id) {
        self._on_send(id)
    }

    pub(crate) fn maintenance(&mut self) {
        self._maintenance()
    }

    pub(crate) fn load(&mut self, path: &str) {
        let mut fp = match File::open(path) {
            Ok(v) => v,
            Err(e) => {
                warn!("Opening persistent file: {} error {}", path, e);
                return
            }
        };

        let mut buf = vec![];
        if let Err(e) = fp.read_to_end(&mut buf) {
            warn!("Failed to read persistent file: {}", e);
            return;
        };

        let val: ciborium::value::Value;
        let reader = cbor::Reader::new(&buf);
        val = ciborium::de::from_reader(reader)
            .map_err(|e| return e)
            .ok()
            .unwrap();

        let root = match val.as_map() {
            Some(v) => v,
            None => return,
        };

        let mut timestamp = SystemTime::UNIX_EPOCH;
        let mut len = 0;

        for (k,v) in root {
            let k = match k.as_text() {
                Some(k) => k,
                None => return,
            };

            match k {
                "timestamp" => {
                    let v = match v.as_integer() {
                        Some(v) => v.try_into().unwrap(),
                        None => return,
                    };
                    timestamp += Duration::from_secs(v);
                },
                "entries" => {
                    let v = match v.as_array() {
                        Some(v) => v,
                        None => return,
                    };

                    if v.len() == 0 {
                        return;
                    }
                    for item in v.iter() {
                        let entry = match KBucketEntry::from_cbor(item) {
                            Some(v) => v,
                            None => return,
                        };
                        self._put(
                            Rc::new(RefCell::new(entry))
                        );
                    }
                    len = v.len();
                },
                _ => return,
            };
        }

        info!("Loaded {} entries from persistent file, it was {} min old", len,
            SystemTime::now().duration_since(timestamp).unwrap().as_secs() / 60
        );
    }

    pub(crate) fn save(&self, path: &str) {
        let mut fp = match File::create(path) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to creating/opening routing table file with error {}", e);
                return;
            }
        };

        let mut entries = vec![];
        for bucket in self.buckets.values() {
            bucket.borrow().entries().iter().for_each(|item| {
                entries.push(item.borrow().to_cbor());
            });
            // TODOO
        }

        let mut val = Value::Map(vec![
            (
                Value::Text(String::from("timestamp")),
                Value::Integer(SystemTime::now().elapsed().unwrap().as_secs().into())
            ),
            (
                Value::Text(String::from("entries")),
                Value::Array(entries)
            )
        ]);

        let mut buf = vec![];
        let writer = cbor::Writer::new(&mut buf);
        let _ = ciborium::ser::into_writer(&mut val, writer);

        if let Err(e) = fp.write_all(&buf) {
            warn!("Failed to write persistent routing table file with error: {}", e);
            return;
        }
        _ = fp.sync_data();
    }

    // The bucket has already been removed from the routing table
    fn _split(&mut self, bucket: &Rc<RefCell<KBucket>>) {
        let mut borrowed = bucket.borrow_mut();
        let prefix = borrowed.prefix();
        let pl = Rc::new(prefix.split_branch(false));
        let ph = Rc::new(prefix.split_branch(true));

        let home_bucket = |p: &Prefix| -> bool {
            p.is_prefix_of(&self.nodeid)
        };

        let mut low  = KBucket::new(pl.clone(), home_bucket(&pl));
        let mut high = KBucket::new(ph.clone(), home_bucket(&ph));

        while let Some(entry) = borrowed.pop() {
            let id = entry.borrow().id().clone();
            match low.prefix().is_prefix_of(&id) {
                true  => low._put(entry),
                false => high._put(entry)
            }
        }

        self.buckets.insert(pl, Rc::new(RefCell::new(low)));
        self.buckets.insert(ph, Rc::new(RefCell::new(high)));
    }

    fn _put(&mut self, input: Rc<RefCell<KBucketEntry>>) {
        let nodeid = input.borrow().id().clone();
        let mut bucket = self.pop_bucket(&nodeid);
        while _needs_split(&bucket, &input) {
            self._split(&bucket);
            bucket = self.pop_bucket(&nodeid);
        }
        bucket.borrow_mut()._put(input);
        let prefix = bucket.borrow().prefix().clone();
        self.buckets.insert(prefix, bucket);
    }

    fn _remove(&mut self, id: &Id) {
        let bucket = self.bucket(id);
        let mut borrowed_mut = bucket.borrow_mut();
        let to_remove = match borrowed_mut.entry(id) {
            Some(v) => v.clone(),
            None => return,
        };
        borrowed_mut._remove_bad_entry(to_remove, true);
    }

    #[inline(always)]
    fn _on_timeout(&mut self, id: &Id) {
        self.bucket(id).borrow_mut().on_timeout(id);
    }

    #[inline(always)]
    fn _on_send(&mut self, id: &Id) {
        self.bucket(id).borrow_mut().on_send(id);
    }

    fn _merge_buckets(&mut self) {
        // TODO:
    }

    pub(crate) fn try_ping_maintenance(&mut self,
        options: PingOption,
        bucket: Rc<RefCell<KBucket>>,
        name: &str
    ) {
        if self.maintenance_tasks.contains_key(bucket.borrow().prefix()) {
            return
        }

        let task = Rc::new(RefCell::new({
            let mut task = Box::new(PingRefreshTask::new(self.dht(), bucket.clone(), options));
            task.set_name(name);
            task.add_listener(Box::new(|_| {}));
            task as Box<dyn Task>
        }));

        task.borrow_mut().set_cloned(task.clone());
        self.maintenance_tasks.insert(bucket.borrow().prefix().clone(), task.clone());

        self.dht().borrow().taskman().borrow_mut().add(task);
    }

    fn _maintenance(&mut self) {
        // Don't spam the checks if we're not receiving anything.
        if as_millis!(self.time_of_last_ping_check) < constants::ROUTING_TABLE_MAINTENANCE_INTERVAL {
            return;
        }

        self.time_of_last_ping_check = SystemTime::now();
        self._merge_buckets();

        let mut buckets: Vec<Rc<RefCell<KBucket>>> = self.buckets.values().map(|v| v.clone()).collect();
        let mut to_push = Vec::new();

        while let Some(bucket) = buckets.pop() {
            let mut to_remove = Vec::new();
            let mut to_adjust = Vec::new();

            { // We use this block to limit the scope of the immutable borrow.
                let borrowed = bucket.borrow();
                let is_full = borrowed.size() >= constants::MAX_ENTRIES_PER_BUCKET;

                for entry in borrowed.entries().iter() {
                    // Remove old entries, ourselves, or bootstrap nodes if the bucket is full
                    if entry.borrow().id() == &*self.nodeid || is_full {
                        to_remove.push(entry.clone());
                        continue;
                    }
                    // Adjust wrong entries that don't fit the bucket's prefix
                    if borrowed.prefix().is_prefix_of(entry.borrow().id()) {
                        to_adjust.push(entry.clone());
                    }
                }
            }
            {
                // We use this block to limit the scope of the mutable borrow.
                let mut borrowed_mut = bucket.borrow_mut();
                while let Some(entry) = to_remove.pop() {
                    _ = borrowed_mut._remove_bad_entry(entry.clone(), true);
                }
                // Fix the wrong entries
                while let Some(entry) = to_adjust.pop() {
                    if let Some(removed) = borrowed_mut._remove_bad_entry(entry.clone(), true) {
                        to_push.push(removed);
                    }
                }
            }

            // If the bucket needs refreshing, run the maintenance ping
            if bucket.borrow().needs_refreshing() {
                let name = format!("PingRefreshing bucket - {}", bucket.borrow().prefix());
                self.try_ping_maintenance(PingOption::ProbeCache, bucket.clone(), &name);
            }
        }

        // Put the adjusted ones to their right buckets.
        while let Some(entry) = to_push.pop() {
            self._put(entry);
        }
    }
}

fn _needs_split(bucket: &Rc<RefCell<KBucket>>, input: &Rc<RefCell<KBucketEntry>>) -> bool {
    if !bucket.borrow().prefix().is_splittable() ||
        !bucket.borrow().is_full() ||
        !input.borrow().reachable() ||
        bucket.borrow().exists(input.borrow().id()) ||
        bucket.borrow().needs_replacement() {
        return false;
    }

    bucket.borrow().prefix()
        .split_branch(true)
        .is_prefix_of(input.borrow().id())
}

impl fmt::Display for RoutingTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nodeId:{}\n", self.nodeid)?;
        write!(f,
            "buckets:{}/ entries:{}\n",
            self.size(),
            self.size_of_entries()
        )?;

        self.buckets.iter().for_each(|(_,v)| {
            _ = write!(f, "* {}", v.borrow());
        });
        Ok(())
    }
}
