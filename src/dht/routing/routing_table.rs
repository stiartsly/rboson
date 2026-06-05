use std::{
    fmt,
    result::Result as SResult,
    sync::{Arc, Mutex},
    cmp::Ordering
};
use rbtree::RBTree;
use libsodium_sys::randombytes_uniform;
use serde::{
    Deserialize, Deserializer,
    Serialize, Serializer,
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
};

use crate::{Id};
use crate::dht::{
    consumer::Consumer,
    rpc::Reachability,
    routing:: {
        Prefix,
        KBucket,
        KBucketEntry,
    },
};

#[derive(Serialize, Deserialize)]
struct PersistedRoutingTable {
    #[serde(rename = "nodeId")]
    node_id: Id,
    timestamp: u64,
    entries: Vec<KBucketEntry>,
}

pub(crate) struct RoutingTable {
    local_id: Id,
    buckets: RBTree<Prefix, Arc<Mutex<KBucket>>>,
}

impl RoutingTable {
    const _MAX_PERSIST_AGE_MILLIS: u64 = 24 * 60 * 60 * 1000;

    pub(crate) fn new(nodeid: Id) -> Self {
        let prefix = Prefix::new();
        let bucket = Arc::new(Mutex::new(KBucket::home_bucket(prefix)));
        let mut bs = RBTree::new();
        bs.insert(prefix, bucket);

        Self {
            local_id: nodeid,
            buckets : bs,
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.buckets.len()
    }

    pub(crate) fn is_home_bucket(&self, p: &Prefix) -> bool {
        p.is_prefix_of(&self.local_id)
    }

    pub(crate) fn local_nodeid(&self) -> &Id {
        &self.local_id
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    pub(crate) fn bucket(&self, target: &Id) -> Arc<Mutex<KBucket>> {
        self.buckets.iter()
            .find(|(k,_)| k.is_prefix_of(target))
            .map(|(_,v)| v.clone())
            .expect("panic: no bucket found, should never happen")
    }

    pub(crate) fn buckets(&self) -> Vec<Arc<Mutex<KBucket>>> {
        self.buckets.values().cloned().collect()
    }

    fn bucket_take(&mut self, target: &Id) -> Arc<Mutex<KBucket>> {
        let key = self.buckets.iter()
            .find(|(k,_v)| k.is_prefix_of(target))
            .map(|(k, _)| k.clone())
            .expect("panic: no bucket found, this should never happen");

        self.buckets.remove(&key).unwrap()
    }

    pub(crate) fn bucket_entry(&self, id: Option<&Id>) -> Option<KBucketEntry> {
        if let Some(id) = id {
            return self.bucket(id)
                .lock().unwrap()
                .entry(Some(id));
        }

        let rand_idx = unsafe {
            randombytes_uniform(self.buckets.len() as u32)
        } as usize;

        self.buckets.iter()
            .nth(rand_idx)
            .map(|(_, bucket)| bucket.lock().unwrap().entry(None))
            .flatten()
    }

    #[allow(unused)]
    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.bucket(id).lock().unwrap().contains(id)
    }

    pub(crate) fn number_of_entries(&self) -> usize {
        self.buckets.iter().map(|(_,v)|
            v.lock().unwrap().size()
        ).sum()
    }

    pub(crate) fn index_of(buckets: &Vec<Arc<Mutex<KBucket>>>, id: &Id) -> usize {
        let mut low = 0usize;
        let mut high = buckets.len() - 1;
        let mut mid = 0usize;
        let mut cmp = Ordering::Equal;

        while low <= high {
            mid = (low + high) >> 1;
            let prefix = buckets[mid].lock().unwrap().prefix().clone();
            cmp = id.cmp(prefix.id());

            match cmp {
                Ordering::Greater => low = mid + 1,
                Ordering::Less => {
                    if mid == 0 {
                        return 0;
                    }
                    high = mid - 1;
                }
                Ordering::Equal => return mid,
            }
        }

        if cmp == Ordering::Less {
            mid.saturating_sub(1)
        } else {
            mid
        }
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

    pub(crate) fn on_request_sent(&mut self, id: &Id) {
        self._on_request_sent(id)
    }

    #[allow(unused)]
    pub(crate) fn on_responded(&mut self, id: &Id, rtt: u64) {
        self.bucket(id).lock().unwrap().on_responded(id, rtt);
    }

    // The bucket has already been removed from the routing table
    fn _split(&mut self, bucket: Arc<Mutex<KBucket>>) {
        let mut locked = bucket.lock().unwrap();
        let prefix = locked.prefix();

        let lp = prefix.split_branch(false);
        let hp = prefix.split_branch(true);

        let mut low  = KBucket::new(lp.clone(), self.is_home_bucket(&lp));
        let mut high = KBucket::new(hp.clone(), self.is_home_bucket(&hp));

        while let Some(entry) = locked.pop() {
            match low.prefix().is_prefix_of(entry.id()) {
                true  => low.put(entry),
                false => high.put(entry)
            }
        }

        self.buckets.insert(lp, Arc::new(Mutex::new(low)));
        self.buckets.insert(hp, Arc::new(Mutex::new(high)));
    }

    fn _put(&mut self, entry: KBucketEntry) {
        let entry_id = entry.id();
        let mut bucket = self.bucket_take(entry_id);

        while Self::needs_split(bucket.clone(), &entry) {
            self._split(bucket);
            bucket = self.bucket_take(entry_id);
        }

        let prefix = {
            let mut locked_bucket = bucket.lock().unwrap();
            locked_bucket.put(entry);
            locked_bucket.prefix().clone()
        };
        self.buckets.insert(prefix, bucket);
    }

    fn needs_split(bucket: Arc<Mutex<KBucket>>, entry: &KBucketEntry) -> bool {
        let locked_bucket = bucket.lock().unwrap();
        if !locked_bucket.prefix().is_splittable() ||
            !locked_bucket.is_full() ||
            !entry.is_reachable() ||
            locked_bucket.contains(entry.id()) {
            return false;
        }

        locked_bucket.prefix()
            .split_branch(true)
            .is_prefix_of(entry.id())
    }

    fn _remove(&mut self, id: &Id) -> Option<KBucketEntry> {
        let bucket = self.bucket(id);
        let to_remove = match bucket.lock().unwrap().entry(Some(id)) {
            Some(v) => v.clone(),
            None => return None,
        };

        let removed = bucket.lock().unwrap()._remove_bad_entry(to_remove, true);
        removed
    }

    fn _on_timeout(&mut self, id: &Id) {
        self.bucket(id).lock().unwrap().on_timeout(id);
    }

    fn _on_request_sent(&mut self, id: &Id) {
        self.bucket(id).lock().unwrap().on_request_sent(id);
    }

    fn _merge_buckets(&mut self) {
        /*
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

            let left_entries = left.lock().unwrap().entries().cloned().collect::<Vec<_>>();
            let right_entries = right.lock().unwrap().entries().cloned().collect::<Vec<_>>() ;
            let mut merged = KBucket::new(parent.clone(), self.is_home_bucket(&parent));
            for entry in left_entries.into_iter().chain(right_entries.into_iter()) {
                merged.put(entry.clone());
            }

            self.buckets.remove(&left_prefix);
            self.buckets.remove(&right_prefix);
            self.buckets.insert(parent, Arc::new(Mutex::new(merged)));

            if idx > 1 {
                idx -= 1;
            }
        }
        */
    }

    pub(crate) fn maintenance(&mut self, bootstrap_ids: &[Id], consumer: Consumer<Arc<Mutex<KBucket>>>) {
        self._merge_buckets();

        for bucket in self.buckets.values() {
            let mut locked = bucket.lock().unwrap();
            locked.cleanup(&self.local_id, bootstrap_ids,
                Consumer::new(move |_entry| {
                    // TODO: Self::put(&mut locked, _entry);
                })
            );

            let need_refreshing  = locked.needs_refreshing();
            let need_replacement = locked.needs_replacement_ping();
            let prefix = locked.prefix().clone();
            drop(locked);

            if need_refreshing || need_replacement {
                log::debug!("Refreshing bucket {}...", prefix);
                consumer.accept(bucket.clone());
            }
        }
    }

    pub(crate) fn save(&self, _path: &str) -> crate::Result<()> {
        /*
        if self.number_of_entries() == 0 {
            return Ok(());
        }

        let mut entries = Vec::with_capacity(self.number_of_entries());
        for (_, bucket) in self.buckets.iter() {
            entries.extend(bucket.lock().unwrap().entries());
        }

        let persisted = PersistedRoutingTable {
            node_id: self.local_id.clone(),
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
            entries,
        };

        let bytes = serde_cbor::to_vec(&persisted)?;
        let file = Path::new(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = file.with_extension("tmp");
        fs::write(&tmp_path, bytes)?;
        fs::rename(&tmp_path, file)?;
        */
        Ok(())
    }

    pub(crate) fn load(&mut self, _path: &str) -> crate::Result<()> {
        /*
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        let persisted: PersistedRoutingTable = serde_cbor::from_slice(&bytes)?;
        if persisted.node_id != self.local_id {
            return Ok(());
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64;
        if now.saturating_sub(persisted.timestamp) > Self::MAX_PERSIST_AGE_MILLIS {
            return Ok(());
        }

        let mut loaded = RoutingTable::new(self.local_id.clone());
        for entry in persisted.entries {
            loaded.put(entry);
        }

        *self = loaded;
        */
        Ok(())
    }
}

impl Serialize for RoutingTable {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer,
    {
        let mut state = se.serialize_struct("RoutingTable", 2)?;
        let mut entries = Vec::with_capacity(self.number_of_entries());
        for (_, bucket) in self.buckets.iter() {
            entries.extend(bucket.lock().unwrap().entries().iter().cloned());
        }

        state.serialize_field("nodeId", &self.local_id)?;
        state.serialize_field("entries", &entries)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for RoutingTable {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where D: Deserializer<'de>,
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
                formatter.write_str("a RoutingTable struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>,
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
        de.deserialize_map(RoutingTableVisitor)
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
