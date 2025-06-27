use std::rc::Rc;
use std::cell::RefCell;
use std::cmp::Ordering;

use crate::{
    Id,
    NodeInfo
};

use crate::dht::{
    kbucket::KBucket,
    kbucket_entry::KBucketEntry,
    routing_table::RoutingTable
};

pub(crate) struct KClosestNodes {
    target: Rc<Id>,
    ni: Rc<NodeInfo>,
    rt: Rc<RefCell<RoutingTable>>,

    entries: Vec<Rc<RefCell<KBucketEntry>>>,
    capacity: usize,

    filter: Box<dyn Fn(&Rc<RefCell<KBucketEntry>>) -> bool>,
}

impl KClosestNodes {
    pub(crate) fn new(target: Rc<Id>,
        ni: Rc<NodeInfo>,
        rt: Rc<RefCell<RoutingTable>>,
        max_entries: usize
    ) -> Self {
        Self::with_filter(
            target,
            ni,
            rt,
            max_entries,
            Box::new(|e: &Rc<RefCell<KBucketEntry>>|
                e.borrow().is_eligible_for_nodes_list()
            )
        )
    }

    pub(crate) fn with_filter<F>(target: Rc<Id>,
        ni: Rc<NodeInfo>,
        rt: Rc<RefCell<RoutingTable>>,
        max_entries: usize,
        filter: F
    ) -> Self
    where F: Fn(&Rc<RefCell<KBucketEntry>>) -> bool + 'static {
        Self {
            target,
            ni,
            rt,
            entries: Vec::new(),
            capacity: max_entries,
            filter: Box::new(filter),
        }
    }

    pub(crate) fn fill(&mut self, include_itself: bool) {
        let mut idx = 0;
        let mut bucket = None;
        let rt = self.rt.clone();
        let binding_rt = rt.borrow();

        for (k,v) in binding_rt.buckets().iter() {
            if k.is_prefix_of(&self.target) {
                bucket = Some(v);
                break;
            }
            idx += 1;
        }

        self.insert_entries(bucket.unwrap());

        let mut low  = idx;
        let mut high = idx;
        let mut iter = binding_rt.buckets().iter();
        let len = binding_rt.buckets().len();

        while self.entries.len() < self.capacity {
            let mut low_bucket  = None;
            let mut high_bucket = None;

            if low > 0 {
                low_bucket = iter.nth(low - 1);
            }
            if high < len - 1{
                high_bucket = iter.nth(high + 1);
            }

            if low_bucket.is_none() && high_bucket.is_none() {
                break;
            } else if low_bucket.is_none() {
                high += 1;
                self.insert_entries(
                    high_bucket.unwrap().1
                );
            } else if high_bucket.is_none() {
                low -= 1;
                self.insert_entries(
                    low_bucket.unwrap().1
                );
            } else {
                let low_bucket = low_bucket.unwrap().1;
                let high_bucket = high_bucket.unwrap().1;

                let ordering = self.target.three_way_compare(
                    &low_bucket.borrow().prefix().last(),
                    &high_bucket.borrow().prefix().first()
                );
                match ordering {
                    Ordering::Less => {
                        low -= 1;
                        self.insert_entries(low_bucket);
                    },
                    Ordering::Greater => {
                        high += 1;
                        self.insert_entries(high_bucket);
                    },
                    Ordering::Equal => {
                        low -= 1;
                        high += 1;
                        self.insert_entries(low_bucket);
                        self.insert_entries(high_bucket);
                    }
                }
            }
        }

        if self.entries.len() < self.capacity {
            // TODO: bootstraps.
        }

        if self.entries.len() < self.capacity && include_itself {
            let bucket_entry = Rc::new(RefCell::new(KBucketEntry::new(
                self.ni.id().clone(),
                self.ni.socket_addr().clone()
            )));
            self.entries.push(bucket_entry);
        }

        self.shave();
    }

    fn insert_entries(&mut self, bucket: &Rc<RefCell<KBucket>>) {
        bucket.borrow().entries().iter().for_each(|item| {
            if (self.filter)(item) {
                self.entries.push(item.clone())
            }
        })
    }

    fn shave(&mut self) {
        self.entries.dedup();
        if self.entries.len() <= self.capacity {
            return;
        }

        self.entries.sort_by(|a, b|
            self.target.three_way_compare(
                a.borrow().id(),
                b.borrow().id()
            )
        );
        _ = self.entries.split_off(self.capacity);
        // Here obsolete list resource would be freed along with
        // all kbucketEntry inside.
    }

    pub(crate) fn as_nodes(&self) -> Vec<Rc<NodeInfo>> {
        self.entries.iter()
            .map(|v| v.borrow().ni())
            .collect()
    }
}
