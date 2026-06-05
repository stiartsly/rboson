use std::{
    fmt,
    time::SystemTime
};
use libsodium_sys::randombytes_uniform;
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
    entries         : Vec<KBucketEntry>,
    last_refreshed  : Option<SystemTime>,
}

impl KBucket {
    pub(crate) const MAX_ENTRIES: usize = 8;
    pub(crate) const REFRESH_INTERVAL: u128 = 15 * 60 * 1000;   // 15 minutes in milliseconds

    pub(crate) fn new(prefix: Prefix, home_bucket: bool) -> Self {
        Self {
            prefix,
            home_bucket,
            entries         : Vec::new(),
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

    pub(crate) fn entries(&self) -> &Vec<KBucketEntry> {
        &self.entries
    }

    #[allow(unused)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.entries.len() >= KBucket::MAX_ENTRIES
    }

    pub(crate) fn contains(&self, id: &Id) -> bool {
        self.entries.iter().position(|entry| entry.id() == id).is_some()
    }

    // bucket.entry(id)     : return the entry for the id;
    // bucket.entry(None)   : return a random entry from the bucket
    pub(crate) fn entry(&self, id: Option<&Id>) -> Option<KBucketEntry> {
        if self.entries.is_empty() {
            return None;
        }

        if let Some(id) = id {
            return self.entries.iter()
                .find(|entry| entry.id() == id).cloned();
        }

        let rand_idx = unsafe {
            randombytes_uniform(self.entries.len() as u32)
        } as usize;
        self.entries.get(rand_idx).cloned()
    }

    pub(crate) fn update_refresh_time(&mut self) {
        self.last_refreshed = Some(SystemTime::now());
    }

    pub(crate) fn needs_refreshing(&self) -> bool {
        let outdated = self.last_refreshed.map_or(true, |v| {
            v.elapsed().unwrap().as_millis() > KBucket::REFRESH_INTERVAL
        });
        outdated && self.entries.iter().any(|v| v.needs_ping())
    }

    pub(crate) fn needs_replacement_ping(&self) -> bool {
        // General DHT node does not support replacement
        false
    }

    pub(crate) fn needs_replacement(&self) -> bool {
        self.entries.iter().any(|v| v.needs_replacement())
    }

    pub(crate) fn pop(&mut self) -> Option<KBucketEntry> {
        self.entries.pop()
    }

    pub(crate) fn put(&mut self, new: KBucketEntry) {
        for item in self.entries.iter_mut() {
            if item.equals(&new) {
                item.merge(new);
                return;
            }
            if item.matches(&new) {
                info!("New node {} claims same ID or IP as {}, might be impersonation attack or IP change.
                    ignoring until old entry times out", new, item);
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

    fn _put_as_main_entry(&mut self, _entry: KBucketEntry) {
        let youngest = match !self.entries.is_empty() {
            true => self.entries.last().cloned(),
            false => None,
        };
        let created_time = _entry.created_time().clone();
        self.entries.push(_entry);

        let Some(youngest) = &youngest else {
            return;
        };

        let unordered = &created_time < youngest.created_time();
        if unordered {
            self.entries.sort_by(|a, b|
                a.created_time().cmp(&b.created_time()));
        }
    }

    pub(crate) fn on_timeout(&mut self, id: &Id) {
        let found = self.entries.iter_mut().find(|item| item.id() == id);
        if let Some(item) = found {
            item.on_timeout();
        }
    }

    pub(crate) fn on_request_sent(&mut self, id: &Id) {
        let found = self.entries.iter_mut().find(|item| item.id() == id);
        if let Some(item) = found {
            item.on_request_sent();
        }
    }

    pub(crate) fn on_responded(&mut self, id: &Id, rtt: u64) {
        let found = self.entries.iter_mut().find(|item| item.id() == id);
        if let Some(item) = found {
            item.on_responded(rtt);
        }
    }

    pub(crate) fn _remove_bad_entry(&mut self, entry: KBucketEntry, force: bool
    ) -> Option<KBucketEntry> {
        let pos = self.entries.iter().position(|item| item.equals(&entry));
        let Some(pos) = pos else {
            return None;
        };
        let Some(item) = self.entries.get(pos).map(|v| v.clone()) else {
            return None;
        };
        if force || item.needs_replacement() {
            self.entries.remove(pos);
        }
        Some(item)
    }

    fn _replace_bad_entry(&mut self, entry: KBucketEntry) {
        let idx = self.entries.iter().position(|entry| entry.needs_replacement());
        if let Some(idx) = idx {
            self.entries.remove(idx);
            self._put_as_main_entry(entry);
        }
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
        for v in self.entries.iter() {
            write!(f, " - {}\n", v)?;
        }
        Ok(())
    }
}
