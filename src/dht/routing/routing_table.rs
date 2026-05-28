use std::fmt;
use std::result::Result as SResult;
use std::sync::{Arc, Mutex};
use rbtree::RBTree;
use libsodium_sys::randombytes_uniform;
use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
};

use crate::{Id};
use crate::dht::{
    rpc::Reachability,
    routing:: {
        KClosestNodes,
        Prefix,
        KBucket,
        KBucketEntry,
    },
};
pub(crate) struct RoutingTable {
    local_id: Id,
    buckets: RBTree<Prefix, Arc<Mutex<KBucket>>>,
}

impl RoutingTable {
    pub(crate) fn new(nodeid: Id) -> Self {
        let buckets = {
            let prefix = Prefix::new();
            let bucket = Arc::new(Mutex::new(KBucket::with_homebucket(prefix)));
            let mut bs = RBTree::new();
            bs.insert(prefix, bucket);
            bs
        };

        Self {
            local_id: nodeid,
            buckets,
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.buckets.len()
    }

    pub(crate) fn is_home_bucket(&self, p: &Prefix) -> bool {
        p.is_prefix_of(&self.local_id)
    }

    pub(crate) fn local_id(&self) -> &Id {
        &self.local_id
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    pub(crate) fn bucket(&self, target: &Id) -> Arc<Mutex<KBucket>> {
        self.buckets.iter()
            .find(|(k,_)| k.is_prefix_of(target))
            .map(|(_,v)| v.clone())
            .expect("panic: no bucket found, this should never happen")
    }

    pub(crate) fn bucket_at(&self, index: usize) -> Option<Arc<Mutex<KBucket>>> {
        self.buckets.iter().nth(index).map(|(_, v)| v.clone())
    }

    pub(crate) fn buckets(&self) -> Vec<Arc<Mutex<KBucket>>> {
        self.buckets.iter().map(|(_, v)| v.clone()).collect()
    }

    fn bucket_take(&mut self, target: &Id) -> Arc<Mutex<KBucket>> {
        let key = self.buckets.iter()
            .find(|(k,_v)| k.is_prefix_of(target))
            .map(|(k, _)| k.clone())
            .expect("panic: no bucket found, this should never happen");

        self.buckets.remove(&key).unwrap()
    }

    pub(crate) fn for_each_bucket(&self, mut f: impl FnMut(Arc<Mutex<KBucket>>)) {
        self.buckets.iter().for_each(|(_,v)| f(v.clone()));
    }

    pub(crate) fn bucket_entry(&self, id: &Id) -> Option<KBucketEntry> {
        self.bucket(id).lock().unwrap().entry(id)
    }

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.bucket(id).lock().unwrap().contains(id)
    }

    pub(crate) fn number_of_entries(&self) -> usize {
        self.buckets.iter().map(|(_,v)|
            v.lock().unwrap().size()
        ).sum()
    }

    pub(crate) fn random_kentry(&self) -> Option<KBucketEntry> {
        let keys = self.buckets.keys().collect::<Vec<_>>();
        let rand = unsafe {
            randombytes_uniform(keys.len() as u32)
        } as usize;

        self.buckets[keys[rand]].lock().unwrap().random_entry()
    }

    pub(crate) fn bucket_of(&self, id: &Id) -> (usize, Arc<Mutex<KBucket>>) {
        self.buckets
            .iter()
            .enumerate()
            .find(|(_, (k, _))| k.is_prefix_of(id))
            .map(|(idx, (_, bucket))| (idx, bucket.clone()))
            .expect("panic: bucket not found, this should never happen")
    }

    pub(crate) fn closest_nodes(
        routing_table: Arc<Mutex<Self>>,
        target: Id,
        expected: usize,
    ) -> KClosestNodes {
        KClosestNodes::new(routing_table, target, expected)
    }

    pub(crate) fn put(&mut self, entry: KBucketEntry) {
        self._put(entry)
    }

    pub(crate) fn remove(&mut self, id: &Id) -> Option<KBucketEntry> {
        self._remove(id)
    }

    pub(crate) fn on_timeout(&mut self, id: &Id) {
        self._on_timeout(id)
    }

    pub(crate) fn on_send(&mut self, id: &Id) {
        self._on_send(id)
    }

    pub(crate) fn on_responded(&mut self, id: &Id, rtt: u64) {
        self.bucket(id).lock().unwrap().on_responded(id, rtt);
    }

    pub(crate) fn maintenance(&mut self) {
        self._merge_buckets();
    }

    // The bucket has already been removed from the routing table
    fn _split(&mut self, bucket: Arc<Mutex<KBucket>>) {
        let mut locked = bucket.lock().unwrap();
        let prefix = locked.prefix();

        let lp = prefix.split_branch(false);
        let hp = prefix.split_branch(true);

        let mut low  = KBucket::new(lp.clone(), self.is_home_bucket(&lp));
        let mut high = KBucket::new(hp.clone(), self.is_home_bucket(&hp));

        while let Some(entry) = locked.pop_first() {
            match low.prefix().is_prefix_of(entry.id()) {
                true  => low._put(entry),
                false => high._put(entry)
            }
        }

        self.buckets.insert(lp, Arc::new(Mutex::new(low)));
        self.buckets.insert(hp, Arc::new(Mutex::new(high)));
    }

    fn _put(&mut self, new_entry: KBucketEntry) {

        let id = new_entry.id();
        let mut bucket = self.bucket_take(id);

        while Self::_needs_split(bucket.clone(), &new_entry) {
            self._split(bucket);
            bucket = self.bucket_take(id);
        }

        {
            bucket.lock().unwrap()._put(new_entry);
        }
        let prefix = bucket.lock().unwrap().prefix().clone();
        self.buckets.insert(prefix, bucket);
    }

    fn _needs_split(bucket: Arc<Mutex<KBucket>>, new_entry: &KBucketEntry) -> bool {
        let locked = bucket.lock().unwrap();
        if !locked.prefix().is_splittable() ||
            !locked.is_full() ||
            !new_entry.is_reachable() ||
            locked.contains(new_entry.id()) {
            return false;
        }

        locked.prefix()
            .split_branch(true)
            .is_prefix_of(new_entry.id())
    }

    fn _remove(&mut self, id: &Id) -> Option<KBucketEntry> {
        let bucket = self.bucket(id);
        let to_remove = match bucket.lock().unwrap().entry(id) {
            Some(v) => v.clone(),
            None => return None,
        };

        let removed = bucket.lock().unwrap()._remove_bad_entry(to_remove, true);
        removed
    }

    fn _on_timeout(&mut self, id: &Id) {
        self.bucket(id).lock().unwrap().on_timeout(id);
    }

    fn _on_send(&mut self, id: &Id) {
        self.bucket(id).lock().unwrap().on_send(id);
    }

    fn _merge_buckets(&mut self) {
        let mut idx = 1;

        while idx < self.buckets.len() {
            let buckets = self.buckets.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
            let left = buckets[idx - 1].clone();
            let right = buckets[idx].clone();

            let (left_prefix, right_prefix, parent, can_merge) = {
                let left_locked = left.lock().unwrap();
                let right_locked = right.lock().unwrap();
                let left_prefix = left_locked.prefix().clone();
                let right_prefix = right_locked.prefix().clone();
                let parent = left_prefix.parent();
                let can_merge = left_prefix.is_sibling_of(&right_prefix)
                    && left_locked.size() + right_locked.size() <= KBucket::MAX_ENTRIES;
                (left_prefix, right_prefix, parent, can_merge)
            };

            if !can_merge {
                idx += 1;
                continue;
            }

            let left_entries = left.lock().unwrap().entries();
            let right_entries = right.lock().unwrap().entries();
            let mut merged = KBucket::new(parent.clone(), self.is_home_bucket(&parent));
            for entry in left_entries.into_iter().chain(right_entries.into_iter()) {
                merged._put(entry);
            }

            self.buckets.remove(&left_prefix);
            self.buckets.remove(&right_prefix);
            self.buckets.insert(parent, Arc::new(Mutex::new(merged)));

            if idx > 1 {
                idx -= 1;
            }
        }
    }

    /*
    pub(crate) fn try_ping_maintenance(&mut self,
        options: PingOption,
        bucket: Arc<Mutex<KBucket>>,
        name: &str
    ) {
        let prefix = bucket.lock().unwrap().prefix().clone();
        if self.maintenance_tasks.contains_key(&prefix) {
            return
        }

        let task = Arc::new(Mutex::new({
            let mut task =PingRefreshTask::new(self.dht(), bucket.clone(), options);
            task.set_name(name);
            task.add_listener(Box::new(|_| {}));
            task as Box<dyn Task>
        }));

        task.lock().unwrap().set_cloned(task.clone());
        self.maintenance_tasks.insert(bucket.borrow().prefix().clone(), task.clone());

        self.dht().borrow().taskman().borrow_mut().add(task);
    }


    fn _maintenance(&mut self) {
        // Don't spam the checks if we're not receiving anything.
        if elapsed_ms!(self.time_of_last_ping_check) < constants::ROUTING_TABLE_MAINTENANCE_INTERVAL {
            return;
        }

        self.time_of_last_ping_check = SystemTime::now();
        self._merge_buckets();

        let mut buckets: Vec<KBucket> = self.buckets.values().map(|v| v.clone()).collect();
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
    */
}

impl Serialize for RoutingTable {
    fn serialize<S>(&self, ser: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = ser.serialize_struct("RoutingTable", 2)?;
        let mut entries = Vec::with_capacity(self.number_of_entries());
        for (_, bucket) in self.buckets.iter() {
            entries.extend(bucket.lock().unwrap().entries());
        }

        state.serialize_field("nodeId", &self.local_id)?;
        state.serialize_field("entries", &entries)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for RoutingTable {
    fn deserialize<D>(des: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            NodeId,
            Entries,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(deserializer)?;
                match key.as_str() {
                    "nodeId" => Ok(Field::NodeId),
                    "entries" => Ok(Field::Entries),
                    _ => Err(de::Error::unknown_field(&key, &["nodeId", "entries"])),
                }
            }
        }

        struct RoutingTableVisitor;

        impl<'de> Visitor<'de> for RoutingTableVisitor {
            type Value = RoutingTable;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct RoutingTable")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut node_id: Option<Id> = None;
                let mut entries: Vec<KBucketEntry> = Vec::new();

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::NodeId => node_id = Some(map.next_value()?),
                        Field::Entries => entries = map.next_value()?,
                    }
                }

                let node_id = node_id.ok_or_else(|| de::Error::missing_field("nodeId"))?;
                let mut table = RoutingTable::new(node_id);
                for entry in entries {
                    table.put(entry);
                }
                Ok(table)
            }
        }

        des.deserialize_map(RoutingTableVisitor)
    }
}

impl fmt::Display for RoutingTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nodeId:{}\n", self.local_id)?;
        write!(f,
            "buckets:{}/ entries:{}\n",
            self.size(),
            self.number_of_entries()
        )?;

        self.buckets.iter().for_each(|(_,v)| {
            _ = write!(f, "* {}", v.lock().unwrap());
        });
        Ok(())
    }
}
