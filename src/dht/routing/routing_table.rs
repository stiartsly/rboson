use std::{
    fmt,
    path::Path,
    cmp::Ordering,
    time::SystemTime,
    fs::{self, File},
    io::{ErrorKind, Error as StdError},
    sync::{Arc, Mutex},
};
use serde::{Deserialize, Serialize};
use rbtree::RBTree;
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
struct SerdeRoutingTable {
    #[serde(rename = "nodeId")]
    nodeid: Id,
    timestamp: u64,
    entries: Vec<KBucketEntry>,
}

pub(crate) struct RoutingTable {
    nodeid  : Id,
    buckets : RBTree<Prefix, Arc<Mutex<KBucket>>>,
    updated : SystemTime,
    saved   : SystemTime,
}

impl RoutingTable {
    const _MAX_PERSIST_AGE_MILLIS: u64 = 24 * 60 * 60 * 1000;

    pub(crate) fn new(nodeid: Id) -> Self {
        let prefix = Prefix::new();
        let bucket = Arc::new(Mutex::new(KBucket::home_bucket(prefix)));
        let mut bs = RBTree::new();
        bs.insert(prefix, bucket);

        Self {
            nodeid  : nodeid,
            buckets : bs,
            updated : SystemTime::UNIX_EPOCH,
            saved   : SystemTime::UNIX_EPOCH,
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.buckets.len()
    }

    pub(crate) fn is_home_bucket(&self, p: &Prefix) -> bool {
        p.is_prefix_of(&self.nodeid)
    }

    pub(crate) fn nodeid(&self) -> &Id {
        &self.nodeid
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
        self._put(entry);
        self.updated = SystemTime::now();
    }

    pub(crate) fn remove(&mut self, id: &Id) -> Option<KBucketEntry> {
        self._remove(id).map(|entry| {
            self.updated = SystemTime::now();
            entry
        })
    }

    pub(crate) fn on_timeout(&self, id: &Id) {
        self._on_timeout(id)
    }

    pub(crate) fn on_request_sent(&self, id: &Id) {
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

        let mut low  = KBucket::new(lp, self.is_home_bucket(&lp));
        let mut high = KBucket::new(hp, self.is_home_bucket(&hp));

        for item in locked.entries().iter().cloned() {
            match lp.is_prefix_of(item.id()) {
                true  => low.put(item),
                false => high.put(item)
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
            let prefix = *bucket.lock().unwrap().prefix();
            self.buckets.remove(&prefix);
        }
        for bucket in to_add {
            let prefix = *bucket.lock().unwrap().prefix();
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

    fn _on_timeout(&self, id: &Id) {
        self.bucket(id).lock().unwrap().on_timeout(id);
    }

    fn _on_request_sent(&self, id: &Id) {
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

    pub(crate) fn maintenance(
        rt: Arc<Mutex<Self>>,
        bootstrap_ids: &[Id],
        consumer: Consumer<Arc<Mutex<KBucket>>>
    ){
        let mut locked_rt = rt.lock().unwrap();
        locked_rt._merge_buckets();
        locked_rt.updated = SystemTime::now();

        let buckets = locked_rt.buckets.values().cloned();
        for bucket in buckets {
            let mut locked = bucket.lock().unwrap();
            locked.cleanup(&locked_rt.nodeid, bootstrap_ids,
                Consumer::new(move |_entry| {
                    unimplemented!()
                    // TODO: Self::put(&mut locked, _entry);
                })
            );

            let needs_refreshing  = locked.needs_refreshing();
            let needs_replacement = locked.needs_replacement_ping();
            let prefix = locked.prefix().clone();
            drop(locked);

            if needs_refreshing || needs_replacement {
                log::debug!("Refreshing bucket {}...", prefix);
                consumer.accept(&bucket);
            }
        }
    }

    pub(crate) fn save(&mut self, path: &Path) -> Result<()> {
        if self.updated <= self.saved {
            return Ok(());
        }

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            File::create(path)?;
        }
        if !path.is_file() {
            return Err(StdError::new(
                ErrorKind::InvalidInput,
                format!("Path {} is not a file", path.display())
            ).into());
        }

        if self.number_of_entries() == 0 {
            return Ok(());
        }

        let mut entries = Vec::with_capacity(self.number_of_entries());
        for item in self.buckets.values() {
            entries.extend(item.lock().unwrap().entries());
        }

        let saved = SystemTime::now();
        let persisted = SerdeRoutingTable {
            nodeid      : self.nodeid,
            timestamp   : crate::as_ms!(saved) as u64,
            entries,
        };

        let bytes = serde_cbor::to_vec(&persisted)?;
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, bytes)?;
        fs::rename(&tmp_path, &path)?;

        self.saved = saved;
        Ok(())
    }

    pub(crate) fn load(&mut self, path: &Path) -> Result<()> {
        const MAX_AGE: u64 = 24 * 60 * 60 * 1000;

        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        let rt: SerdeRoutingTable = serde_cbor::from_slice(&bytes)?;
        if  rt.nodeid != self.nodeid {
            return Ok(());
        }

        let now = crate::as_ms!(SystemTime::now()) as u64;
        if now - rt.timestamp > MAX_AGE{
            return Ok(());
        }

        for item in rt.entries {
            self._put(item);
        }
        Ok(())
    }
}

impl fmt::Display for RoutingTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nodeId:{}\n", self.nodeid)?;
        write!(f,
            "buckets:{}/ entries:{}\n",
            self.size(),
            self.number_of_entries()
        )?;

        self.buckets.values().for_each(|v| {
            _ = write!(f, "* {}", v.lock().unwrap());
        });
        Ok(())
    }
}
