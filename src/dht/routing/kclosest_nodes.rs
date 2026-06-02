use std::{
    sync::{Arc, Mutex},
    cmp::Ordering
};

use crate::{Id, NodeInfo};
use crate::dht::{
    dht::DHT,
    routing::{
        KBucket,
        KBucketEntry,
        RoutingTable
    }
};

pub(crate) struct KClosestNodes {
    local_id: Id,
    buckets : Vec<Arc<Mutex<KBucket>>>,
    target  : Id,
    capacity: usize,
    entries : Vec<KBucketEntry>,
    filter  : Box<dyn Fn(&KBucketEntry) -> bool>
}

impl KClosestNodes {
    pub(crate) fn new(
        dht: &DHT,
        target: Id,
        capacity: usize
    ) -> Self {
        let local_id = dht.rt().local_nodeid().clone();
        let buckets  = dht.rt().buckets();
        Self {
            filter: Self::default_filter(local_id.clone()),
            local_id,
            buckets,
            target,
            capacity,
            entries: Vec::with_capacity(capacity + KBucket::MAX_ENTRIES),
        }
    }

    fn default_filter(local_id: Id) -> Box<dyn Fn(&KBucketEntry) -> bool> {
        Box::new(move |entry: &KBucketEntry| {
            entry.eligible_for_nodes_list() && entry.id() != &local_id
        })
    }

    #[cfg(test)]
    pub(crate) fn target(&self) -> &Id {
        &self.target
    }

    #[cfg(test)]
    pub(crate) fn size(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn is_full(&self) -> bool {
        self.entries.len() >= self.capacity
    }

    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[KBucketEntry] {
        &self.entries
    }

    pub(crate) fn set_filter<F>(&mut self, cb: F)
    where F: Fn(&KBucketEntry) -> bool + 'static
    {
        let local_id = self.local_id.clone();
        self.filter = Box::new(move |entry: &KBucketEntry| {
            cb(entry) && entry.id() != &local_id
        });
    }

    pub(crate) fn fill(&mut self) {
        let buckets = self.buckets.clone();
        if buckets.is_empty() {
            return;
        }

        let idx = RoutingTable::index_of(&buckets, &self.target);
        let bucket = buckets[idx].clone();
        self.add_entries(&bucket);

        let mut low  = idx;
        let mut high = idx;
        let len = buckets.len();

        while self.entries.len() < self.capacity {
            let low_bucket = if low > 0 {
                Some(buckets[low - 1].clone())
            } else {
                None
            };
            let high_bucket = if high < len - 1 {
                Some(buckets[high + 1].clone())
            } else {
                None
            };

            if low_bucket.is_none() && high_bucket.is_none() {
                break;
            }

            if low_bucket.is_none() {
                high += 1;
                self.add_entries(high_bucket.as_ref().unwrap());
            } else if high_bucket.is_none() {
                low -= 1;
                self.add_entries(low_bucket.as_ref().unwrap());
            } else {
                let low_bucket  = low_bucket.unwrap();
                let high_bucket = high_bucket.unwrap();
                let low_prefix  = low_bucket.lock().unwrap().prefix().clone();
                let high_prefix = high_bucket.lock().unwrap().prefix().clone();

                let ordering = self.target.three_way_compare(
                    &low_prefix.last(),
                    &high_prefix.first()
                );
                match ordering {
                    Ordering::Less => {
                        low -= 1;
                        self.add_entries(&low_bucket);
                    },
                    Ordering::Greater => {
                        high += 1;
                        self.add_entries(&high_bucket);
                    },
                    Ordering::Equal => {
                        low -= 1;
                        high += 1;
                        self.add_entries(&low_bucket);
                        self.add_entries(&high_bucket);
                    }
                }
            }
        }
        self.shave();
    }

    fn add_entries(&mut self, bucket: &Arc<Mutex<KBucket>>) {
        let entries = bucket.lock().unwrap().entries();
        for item in entries {
            if (self.filter)(&item) {
                self.entries.push(item)
            }
        };
    }

    fn shave(&mut self) {
        self.entries.sort_by(|e1, e2|
            self.target.three_way_compare(e1.id(), e2.id())
        );

        if self.entries.len() <= self.capacity {
            return;
        }
        // split off the entries that exceed the capacity, and drop them
        // to free the resource.
        _ = self.entries.split_off(self.capacity);
    }
}

impl Into<Vec<KBucketEntry>> for KClosestNodes {
    fn into(self) -> Vec<KBucketEntry> {
        self.entries
    }
}

impl Into<Vec<NodeInfo>> for KClosestNodes {
    fn into(self) -> Vec<NodeInfo> {
        self.entries.into_iter()
            .map(|v| v.as_ref().clone())
            .collect()
    }
}
