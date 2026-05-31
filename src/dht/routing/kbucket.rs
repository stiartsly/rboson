use std::{
    fmt,
    time::SystemTime
};
use rbtree::RBTree;
use libsodium_sys::randombytes_uniform;
use log::info;

use crate::Id;
use crate::dht::{
    rpc::Reachability,
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
    prefix      : Prefix,
    home_bucket : bool,
    entries     : RBTree<Id, KBucketEntry>,
    last_refreshed: SystemTime,
}

impl KBucket {
    pub(crate) const MAX_ENTRIES: usize = 8;
    pub(crate) const REFRESH_INTERVAL: u128 = 15 * 60 * 1000;

    pub(crate) fn new(prefix: Prefix, home_bucket: bool) -> Self {
        Self {
            prefix,
            home_bucket,
            entries: RBTree::new(),
            last_refreshed: SystemTime::UNIX_EPOCH,
        }
    }

    pub(crate) fn with_homebucket(prefix: Prefix) -> Self {
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

    #[allow(unused)]
    pub(crate) fn is_full(&self) -> bool {
        self.entries.len() >= KBucket::MAX_ENTRIES
    }

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.entries.contains_key(id)
    }

    pub(crate) fn entry(&self, id: Option<&Id>) -> Option<KBucketEntry> {
        if let Some(id) = id {
            return self.entries.get(id).cloned();
        }
        if self.entries.is_empty() {
            return None;
        }

        let rand_idx = unsafe {
            randombytes_uniform(self.entries.len() as u32)
        } as usize;

        self.entries.iter()
            .nth(rand_idx)
            .map(|(_, entry)| entry.clone())
    }

    pub(crate) fn update_refresh_time(&mut self) {
        self.last_refreshed = SystemTime::now()
    }

    pub(crate) fn needs_refreshing(&self) -> bool {
        crate::elapsed_ms!(&self.last_refreshed) > KBucket::REFRESH_INTERVAL
            && self.find_any(|v| v.needs_ping()).is_some()
    }

    pub(crate) fn pop(&mut self) -> Option<KBucketEntry> {
        self.entries.pop_first().map(|(_,v)|v)
    }

    pub(crate) fn _put(&mut self, new: KBucketEntry) {
        let mut matched = None;

        for (k, v) in self.entries.iter() {
            if v.eq(&new) {
                matched = Some(k.clone());
                break;
            }
            if v.matches(&new) {
                info!("New node {} claims same ID or IP as {}, might be impersonation attack or IP change.
                    ignoring until old entry times out", new, v);
                return;
            }
        }
        if let Some(id) = matched {
            let existing = self.entries.get_mut(&id).unwrap();
            existing.merge(new);
            return;
        }

        let entry_id = new.id().clone();
        if new.is_reachable() {
            // insert to the list if it still has room
            if self.entries.len() < KBucket::MAX_ENTRIES {
                self.entries.insert(entry_id, new);
                return;
            }

            // Try to replace the bad entry
            self._replace_bad_entry(new);
        }
    }

    pub(crate) fn on_timeout(&mut self, id: &Id) {
        if let Some(item) = self.entries.get_mut(id) {
            item.on_timeout();
        }
    }

    pub(crate) fn on_send(&mut self, id: &Id) {
        if let Some(item) = self.entries.get_mut(id) {
            item.on_request_sent();
        }
    }

    pub(crate) fn on_responded(&mut self, id: &Id, rtt: u64) {
        if let Some(item) = self.entries.get_mut(id) {
            item.on_responded(rtt);
        }
    }

    pub(crate) fn _remove_bad_entry(&mut self, entry: KBucketEntry, force: bool
    ) -> Option<KBucketEntry> {
        if force || entry.needs_replacement() {
            self.entries.remove(entry.id())
        } else {
            None
        }
    }

    fn _replace_bad_entry(&mut self, new_entry: KBucketEntry) {
        let mut bad_removed = None;
        for (k,v) in self.entries.iter() {
            if v.needs_replacement() {
                bad_removed = Some(k.clone());
                break;
            }
        }
        if let Some(bad_id) = bad_removed {
            self.entries.remove(&bad_id);
            self.entries.insert(new_entry.id().clone(), new_entry);
        }
    }

    fn find_any<P>(&self, mut test: P) -> Option<KBucketEntry>
    where P: FnMut(&KBucketEntry) -> bool {
        for (_,v) in self.entries.iter() {
            if test(v) {
                return Some(v.clone());
            }
        }
        None
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
        for (_,v) in self.entries.iter() {
            write!(f, " - {}\n", v)?;
        }
        Ok(())
    }
}
