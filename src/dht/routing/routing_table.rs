use std::{
    fmt,
    fs,
    io::ErrorKind,
    time::SystemTime,
    sync::{Arc, Mutex},
    cmp::Ordering,
    path::{Path},
};
use rbtree::RBTree;
use serde::{Deserialize, Serialize};
use log::debug;

use crate::{Id, Result};
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

    pub(crate) fn bucket_entry(&self, id: &Id) -> Option<KBucketEntry> {
        self.bucket(id).lock().unwrap().entry(Some(id))
    }

    pub(crate) fn random_entry(&self) -> Option<KBucketEntry> {
        self.bucket(&Id::random()).lock().unwrap().entry(None)
    }

    #[allow(unused)]
    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.bucket(id).lock().unwrap().contains(id)
    }

    pub(crate) fn number_of_entries(&self) -> usize {
        self.buckets.values().map(|v| v.lock().unwrap().size()).sum()
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
        let locked = bucket.lock().unwrap();
        let prefix = locked.prefix();

        let lp = prefix.split_branch(false);
        let hp = prefix.split_branch(true);

        let mut low  = KBucket::new(lp.clone(), self.is_home_bucket(&lp));
        let mut high = KBucket::new(hp.clone(), self.is_home_bucket(&hp));

        for entry in locked.entries().iter().cloned() {
            match prefix.is_prefix_of(entry.id()) {
                true  => low.put(entry),
                false => high.put(entry)
            }
        }
        drop(locked);

        self.modify(
            vec![bucket],
            vec![Arc::new(Mutex::new(low)), Arc::new(Mutex::new(high))]
        );
    }

    fn modify(&mut self,
        to_remove: Vec<Arc<Mutex<KBucket>>>,
        to_add: Vec<Arc<Mutex<KBucket>>>
    ) {
        for bucket in to_remove {
            let prefix = bucket.lock().unwrap().prefix().clone();
            self.buckets.remove(&prefix);
        }
        for bucket in to_add {
            let prefix = bucket.lock().unwrap().prefix().clone();
            self.buckets.insert(prefix, bucket);
        }
    }

    fn _put(&mut self, entry: KBucketEntry) {
        let entry_id = entry.id();
        let mut bucket = self.bucket(entry_id);

        while Self::needs_split(&bucket, &entry) {
            self._split(bucket);
            bucket = self.bucket(entry_id);
        }
        bucket.lock().unwrap().put(entry);
    }

    fn needs_split(bucket: &Arc<Mutex<KBucket>>, entry: &KBucketEntry) -> bool {
        let locked = bucket.lock().unwrap();
        if !locked.prefix().is_splittable() ||
            !locked.is_full() ||
            !entry.is_reachable() ||
            locked.contains(entry.id()) {
            return false;
        }

        locked.prefix()
            .split_branch(true)
            .is_prefix_of(entry.id())
    }

    fn _remove(&self, id: &Id) -> Option<KBucketEntry> {
        let bucket = self.bucket(id);
        let mut locked = bucket.lock().unwrap();

        let entry = locked.entry(Some(id));
        let Some(to_remove) = entry else {
            return None;
        };
        locked._remove_bad_entry(to_remove, true)
    }

    fn _on_timeout(&mut self, id: &Id) {
        self.bucket(id).lock().unwrap().on_timeout(id);
    }

    fn _on_request_sent(&mut self, id: &Id) {
        self.bucket(id).lock().unwrap().on_request_sent(id);
    }

    //
	// Attempts to merge adjacent sibling buckets when their combined size
    // does not exceed the maximum allowed.
	// This helps reduce fragmentation and maintain efficient bucket structure.
	//
    fn _merge_buckets(&mut self) {
        debug!("Trying to merge buckets({})... ", self.buckets.len());
        let mut idx = 0;
        while idx < self.buckets.len() {
            let buckets = self.buckets.iter()
                    .map(|(_, v)| v.clone())
                    .collect::<Vec<_>>();

            idx += 1;
            if idx < 1 {
                continue;
            }
            if idx >= buckets.len() {
                break;
            }

            let l = buckets[idx - 1].clone();
            let r = buckets[idx].clone();

            let locked_l = l.lock().unwrap();
            let locked_r = r.lock().unwrap();

            if !locked_l.prefix().is_sibling_of(&locked_r.prefix()) {
                let effective_sz1 = locked_l.filter(|e| e.removable_without_replacement());
                let effective_sz2 = locked_r.filter(|e| e.removable_without_replacement());

                if effective_sz1 + effective_sz2 <= KBucket::MAX_ENTRIES {
                    debug!("Merging buckets {} and {}...",
                        locked_l.prefix(),
                        locked_r.prefix()
                    );

                    let prefix = locked_l.prefix().parent();
                    let is_home_bucket = self.is_home_bucket(&prefix);
                    let mut new_bucket = KBucket::new(prefix, is_home_bucket);

                    for entry in locked_l.entries().iter().cloned() {
                        new_bucket.put(entry);
                    }
                    for entry in locked_r.entries().iter().cloned() {
                        new_bucket.put(entry);
                    }

                    self.modify(
                        vec![l.clone(), r.clone()],
                        vec![Arc::new(Mutex::new(new_bucket))]
                    );

                    idx -= 2; // Adjust index to re-check after merge
                }
            }
            debug!("Finished merge buckets({})... ", self.buckets.len());
        }
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

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        if self.number_of_entries() == 0 {
            return Ok(());
        }

        let mut entries = Vec::with_capacity(self.number_of_entries());
        for (_, item) in self.buckets.iter() {
            entries.extend(item.lock().unwrap().entries());
        }

        let persisted = PersistedRoutingTable {
            node_id     : self.local_id.clone(),
            timestamp   : crate::as_ms!(SystemTime::now()) as u64,
            entries,
        };

        let bytes = serde_cbor::to_vec(&persisted)?;
        let file = path.to_path_buf();
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = file.with_extension("tmp");
        fs::rename(&file, &tmp_path)?;
        fs::write (&tmp_path, bytes)?;
        fs::rename(&tmp_path, &file)?;

        Ok(())
    }

    pub(crate) fn load(&mut self, path: &Path) -> Result<()> {
        const MAX_AGE: u64 = 24 * 60 * 60 * 1000;

        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        let persisted: PersistedRoutingTable = serde_cbor::from_slice(&bytes)?;
        if persisted.node_id != self.local_id {
            return Ok(());
        }

        let now = crate::as_ms!(SystemTime::now()) as u64;
        if now - persisted.timestamp > MAX_AGE{
            return Ok(());
        }

        for entry in persisted.entries {
            self.put(entry);
        }
        Ok(())
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
