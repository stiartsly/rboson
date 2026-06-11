use std::{
    fmt,
    time::SystemTime
};
use libsodium_sys::randombytes_uniform;
use rbtree::RBTree;
use log::info;

use crate::Id;
use crate::dht::{
    rpc::Reachability,
    consumer::Consumer,
    routing::{Prefix, KBucketEntry},
};

/**
 * A KBucket is just a list of KBucketEntry objects.
 *
 * The list is sorted by time last seen : The first element is the least
 * recently seen, the last the most recently seen.
 *
 * CAUTION:
 *   All methods name leading with _ means that method will WRITE the
 *   list, it can only be called inside the routing table's
 *   pipeline processing.
 *
 *   Due the heavy implementation the stream operations are significant
 *   slow than the for-loops. so we should avoid the stream operations
 *   on the KBucket entries and the cache entries, use for-loop instead.
 */

pub(crate) struct KBucket {
    prefix          : Prefix,
    home_bucket     : bool,
    entries         : RBTree<SystemTime, KBucketEntry>,
    last_refreshed  : Option<SystemTime>,
}

impl KBucket {
    pub(crate) const MAX_ENTRIES: usize = 8;
    pub(crate) const REFRESH_INTERVAL: u128 = 15 * 60 * 1000;   // 15 minutes in milliseconds

    pub(crate) fn new(prefix: Prefix, home_bucket: bool) -> Self {
        Self {
            prefix,
            home_bucket,
            entries         : RBTree::new(),
            last_refreshed  : None,
        }
    }

    pub(crate) fn home_bucket(prefix: Prefix) -> Self {
        Self::new(prefix, true)
    }

    pub(crate) fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    #[allow(unused)]
    pub(crate) fn is_home_bucket(&self) -> bool {
        self.home_bucket
    }

    pub(crate) fn size(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn entries(&self) -> Vec<KBucketEntry> {
        self.entries.values().cloned().collect()
    }

    #[allow(unused)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.entries.len() >= KBucket::MAX_ENTRIES
    }

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.entries.iter().any(|(_, entry)| entry.id() == id)
    }

    // bucket.entry(id)     : return the entry for the id;
    // bucket.entry(None)   : return a random entry from the bucket
    pub(crate) fn entry(&self, id: Option<&Id>) -> Option<KBucketEntry> {
        if self.entries.is_empty() {
            return None;
        }

        if let Some(id) = id {
            return self.entries.iter()
                .find(|(_, entry)| entry.id() == id)
                .map(|(_, v)| v.clone());
        }

        let pos = unsafe {
            randombytes_uniform(self.entries.len() as u32)
        } as usize;
        self.entries.values().nth(pos).cloned()
    }

    pub(crate) fn update_refresh_time(&mut self) {
        self.last_refreshed = Some(SystemTime::now());
    }

    pub(crate) fn needs_refreshing(&self) -> bool {
        let needs_ping = self.entries.iter().any(|(_,v)|v.needs_ping());
        let needs_refresh = self.last_refreshed.map_or(true, |v| {
            crate::elapsed_ms!(v) > KBucket::REFRESH_INTERVAL
        });
        needs_refresh && needs_ping
    }

    // General DHT node does not support replacement
    pub(crate) fn needs_replacement_ping(&self) -> bool { false }

    #[allow(unused)]
    pub(crate) fn needs_replacement(&self) -> bool {
        self.entries.iter().any(|(_, v)| v.needs_replacement())
    }

    pub(crate) fn put(&mut self, new: KBucketEntry) {
        for (_, v) in self.entries.iter_mut() {
            if v.equals(&new) {
                v.merge(new);
                return;
            }
            if v.matches(&new) {
                info!("New node {} claims same ID or IP as {}, might be impersonation attack or IP change.
                    ignoring until old entry times out", new, v);
                return;
            }
        }
        if new.is_reachable() {
            // insert to the list if it still has room
            if self.entries.len() < KBucket::MAX_ENTRIES {
                self._put_as_main_entry(new);
                return;
            }

            // Try to replace the bad entry
            self._replace_bad_entry(new);

            // When bucket full and new reachable entry arrives, Kademlia(original paper) pings the
			// oldest/least-recent when full; if unresponsive, replace from cache, else cache the new one.
			// now we reset the last refresh timestamp
			// This will force a refresh to run PingRefreshTask with probe replacement on the current bucket
			// Assumes PingRefreshTask pings least-recent-seen entries for LRS eviction.
			self.last_refreshed = None;
        }
    }

    fn _put_as_main_entry(&mut self, entry: KBucketEntry) {
        let created_time = *entry.created_time();
        self.entries.insert(created_time, entry);
    }

    fn _replace_bad_entry(&mut self, entry: KBucketEntry) {
        let key = self.entries.iter()
            .find(|(_,v)| v.needs_replacement())
            .map(|(k,_)| k.clone());

        if let Some(ref key) = key {
            self.entries.remove(key);
        } else {
            self.entries.pop_last();
        }

        self._put_as_main_entry(entry);
    }

    pub(crate) fn on_timeout(&mut self, id: &Id) {
        let found = self.entries.iter_mut().find(|(_, v)| v.id() == id);
        if let Some((_, v)) = found {
            v.on_timeout();
        }
    }

    pub(crate) fn on_request_sent(&mut self, id: &Id) {
        let found = self.entries.iter_mut().find(|(_, v)| v.id() == id);
        if let Some((_, v)) = found {
            v.on_request_sent();
        }
    }

    pub(crate) fn on_responded(&mut self, id: &Id, rtt: u64) {
        let found = self.entries.iter_mut().find(|(_, v)| v.id() == id);
        if let Some((_, v)) = found {
            v.on_responded(rtt);
        }
    }

    pub(crate) fn _remove_bad_entry(&mut self, entry: KBucketEntry, force: bool
    ) -> Option<KBucketEntry> {
        let test_cb = |v: &KBucketEntry| {
            v.equals(&entry) && (force || v.needs_replacement())
        };
        let key = self.entries.iter()
                    .find(|(_, v)|test_cb(v))
                    .map(|(k, _)| k.clone());

        key.map(|ref k| self.entries.remove(k)).flatten()
    }

    pub(crate) fn filter<F>(&self, test: F) -> usize
    where F: Fn(&KBucketEntry) -> bool,
    {
        self.entries.iter().filter(|(_, v)| test(v)).count()
    }

    pub(crate) fn cleanup(&mut self,
        _local_id: &Id,
        _bootstrap_ids: &[Id],
        _dropped_handler: Consumer<KBucketEntry>) {
        unimplemented!()
    }
}

impl fmt::Display for KBucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Prefix:{}", self.prefix)?;
        if self.home_bucket {
            write!(f, "[Home]")?;
        }
        write!(f, "\n")?;
        if self.entries.is_empty() {
            write!(f, " entries[N/A]\n")?;
        } else {
            write!(f, " entries[{}]\n", self.entries.len())?;
        }
        for v in self.entries.values() {
            write!(f, " - {}\n", v)?;
        }
        Ok(())
    }
}
