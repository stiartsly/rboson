use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::SystemTime;

use rbtree::RBTree;
use libsodium_sys::randombytes_uniform;
use log::info;

use crate::{
    elapsed_ms,
    Id,
    Prefix,
    core::node_info::Reachable,
    dht::{
        constants,
        kbucket_entry::KBucketEntry
    }
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
    prefix: Rc<Prefix>,
    home_bucket: bool,

    entries: RBTree<Id, Rc<RefCell<KBucketEntry>>>,
    youngest: Option<Rc<RefCell<KBucketEntry>>>,  // youngest one.

    last_refreshed: SystemTime,
}

impl KBucket {
    pub(crate) fn new(prefix: Rc<Prefix>, home_bucket: bool) -> Self {
        Self {
            prefix,
            home_bucket,
            entries: RBTree::new(),
            youngest: None,

            last_refreshed: SystemTime::UNIX_EPOCH,
        }
    }

    pub(crate) fn with_homebucket(prefix: Rc<Prefix>) -> Self {
        Self::new(prefix, true)
    }

    pub(crate) fn prefix(&self) -> &Rc<Prefix> {
        &self.prefix
    }

    pub(crate) fn size(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.entries.len() >= constants::MAX_ENTRIES_PER_BUCKET
    }

    pub(crate) fn random_entry(&self) -> Option<Rc<RefCell<KBucketEntry>>> {
        let keys: Vec<_> = self.entries.keys().collect();
        if keys.is_empty() {
            return None;
        }

        let rand = unsafe {
            randombytes_uniform(keys.len() as u32)
        } as usize;

        self.entries.get(keys[rand]).map(|v|v.clone())
    }

    pub(crate) fn entry(&self, target: &Id) -> Option<Rc<RefCell<KBucketEntry>>> {
        self.entries.get(target).map(|v|v.clone())
    }

    pub(crate) fn entries(&self) -> Vec<Rc<RefCell<KBucketEntry>>> {
        self.entries.values().cloned().collect()
    }

    pub(crate) fn pop(&mut self) -> Option<Rc<RefCell<KBucketEntry>>> {
        self.entries.pop_first().map(|(_,v)|v)
    }

    pub(crate) fn exists(&self, id: &Id) -> bool {
        self.entries.contains_key(id)
    }

    pub(crate) fn needs_refreshing(&self) -> bool {
        elapsed_ms!(&self.last_refreshed) > constants::BUCKET_REFRESH_INTERVAL
            && self.find_any(|v| v.borrow().needs_ping()).is_some()
    }

    pub(crate) fn needs_replacement(&self) -> bool {
        self.find_any(|v| v.borrow().needs_replacement())
            .is_some()
    }

    pub(crate) fn update_refresh_time(&mut self) {
        self.last_refreshed = SystemTime::now()
    }

    pub(crate) fn _put(&mut self, entry: Rc<RefCell<KBucketEntry>>) {
        if let Some(item) = self.entries.get_mut(entry.borrow().id()) {
            if item.borrow().equals(&entry.borrow()) {
                item.borrow_mut().merge(entry.clone());
                return;
            }

            // NodeInfo id and address conflict
            // Log the conflict and keep the existing entry
            if item.borrow().matches(&entry.borrow()) {
                info!("New node {} claims same ID or IP as {}, might be impersonation attack or IP change.
                    ignoring until old entry times out", entry.borrow(), item.borrow());
                return;
            }
        }

        let entry_id = entry.borrow().id().clone();
        if entry.borrow().reachable() {
            // insert to the list if it still has room
            if self.entries.len() < constants::MAX_ENTRIES_PER_BUCKET {
                self.entries.insert(entry_id, entry.clone());
                self.youngest = Some(entry);
                return;
            }

            // Try to replace the bad entry
            if self._replace_bad_entry(entry.clone()) {
                return;
            }

            let youngest = match self.youngest.as_ref() {
                Some(v) => v,
                None => return
            };

            if entry.borrow().created_time() > youngest.borrow().created_time() {
                self.entries.replace_or_insert(entry_id, entry.clone());
                self.youngest = Some(entry);
            }
        }
    }

    pub(crate) fn on_timeout(&mut self, id: &Id) {
        if let Some(item) = self.entries.get(id) {
            item.borrow_mut().signal_request_timeout();

            // NOTICE: Test only - merge buckets
            //   remove when the entry needs replacement
            // _removeIfBad(entry, false);
            #[cfg(debug_assertions)] {
                _ = self._remove_bad_entry(item.clone(), false);
            }

            // NOTICE: Product
            //   only removes the entry if it is bad
            #[cfg(not(debug_assertions))] {
                self.entries.remove(id);
            }
        }
    }

    pub(crate) fn on_send(&mut self, id: &Id) {
        if let Some(item) = self.entries.get(id) {
            item.borrow_mut().signal_request();
        }
    }

    pub(crate) fn _remove_bad_entry(&mut self, entry: Rc<RefCell<KBucketEntry>>, force: bool
    ) -> Option<Rc<RefCell<KBucketEntry>>> {
        if force || entry.borrow().needs_replacement() {
            self.entries.remove(entry.borrow().id())
        } else {
            None
        }
    }

    fn _replace_bad_entry(&mut self, new_entry: Rc<RefCell<KBucketEntry>>) -> bool {
        let mut replaced = false;
        for (_,v) in self.entries.iter_mut() {
            if v.borrow().needs_replacement() {
                v.borrow_mut().merge(new_entry);
                replaced = true;
                break;
            }
        }
        replaced
    }

    fn find_any<P>(&self, mut predicate: P) -> Option<Rc<RefCell<KBucketEntry>>>
    where P: FnMut(&Rc<RefCell<KBucketEntry>>) -> bool {
        self.entries.iter()
            .find(|(_,v)| predicate(v))
            .map(|(_,v)|v.clone())
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
            write!(f, " - {}\n", v.borrow())?;
        }
        Ok(())
    }
}
