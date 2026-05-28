use std::sync::{Arc, Mutex};
use std::cmp::Ordering;

use crate::{Id, NodeInfo};
use crate::dht::routing::{
    KBucket,
    KBucketEntry,
    RoutingTable
};
pub(crate) struct KClosestNodes {
    rt      : Arc<Mutex<RoutingTable>>,
    target  : Id,
    capacity: usize,
    entries : Vec<KBucketEntry>,
    filter  : Box<dyn Fn(&KBucketEntry) -> bool>
}

impl KClosestNodes {
    pub(crate) fn new(
        routing_table: Arc<Mutex<RoutingTable>>,
        target: Id,
        capacity: usize
    ) -> Self {
        let cloned = routing_table.clone();

        Self {
            rt: routing_table,
            target,
            capacity,
            entries: Vec::new(),
            filter: Box::new(move |e: &KBucketEntry| {
                let rt = cloned.lock().unwrap();
                e.eligible_for_nodes_list() && e.id() != rt.local_id()
            }),
        }
    }

    pub(crate) fn target(&self) -> &Id {
        &self.target
    }

    pub(crate) fn size(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_full(&self) -> bool {
        self.entries.len() >= self.capacity
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.entries.len() == self.capacity
    }

    pub(crate) fn set_filter<F>(&mut self, filter: F)
    where
        F: Fn(&KBucketEntry) -> bool + 'static
    {
        self.filter = Box::new(filter);
    }

    pub(crate) fn filter<F>(&mut self, filter: F) -> &mut Self
    where
        F: Fn(&KBucketEntry) -> bool + 'static
    {
        let cloned = self.rt.clone();
        self.filter = Box::new(move |entry: &KBucketEntry| {
            let rt = cloned.lock().unwrap();
            filter(entry) && entry.id() != rt.local_id()
        });
        self
    }

    pub(crate) fn fill(&mut self) -> &mut Self {
        self.entries.clear();

        let empty = {
            let locked_rt = self.rt.lock().unwrap();
            locked_rt.buckets().is_empty()
        };

        if empty {
            return self;
        }

        let (idx, buckets) = {
            let locked_rt = self.rt.lock().unwrap();
            let (idx, _) = locked_rt.bucket_of(&self.target);
            let buckets = locked_rt.buckets();
            (idx, buckets)
        };

        self.add_entries(&buckets[idx]);

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
                let low_bucket = low_bucket.unwrap();
                let high_bucket = high_bucket.unwrap();

                let low_prefix = low_bucket.lock().unwrap().prefix().clone();
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
        self
    }

    pub(crate) fn entries(&self) -> &Vec<KBucketEntry> {
        &self.entries
    }

    pub(crate) fn nodes(&self) -> Vec<NodeInfo> {
        self.entries.iter().map(|entry| entry.as_ref().clone()).collect()
    }

    fn add_entries(&mut self, bucket: &Arc<Mutex<KBucket>>) {
        bucket.lock().unwrap().entries().iter().for_each(|item| {
            if (self.filter)(item) {
                self.entries.push(item.clone())
            }
        });
    }

    fn shave(&mut self) {
        self.entries.sort_by(|a, b|
            self.target.three_way_compare(a.id(), b.id())
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
