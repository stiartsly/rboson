use std::sync::{Arc, Mutex};

use crate::{Id, NodeInfo};
use crate::dht::{
    utils::{is_any_unicast, is_bogon},
    dht::DHT,
    node_entry::{Reachability, NodeEntry},
    rpccall::RpcCall,
    routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
    },
    msg::{
        msg::Body,
        lookup_rsp::LookupResponse
    },
    task::{
        closest_set::ClosestSet,
        closest_candidates::ClosestCandidates,
        candidate_node::CandidateNode,
        task::TaskData,
    }
};

pub(crate) struct LookupTaskData {
    target: Id,
    closest: ClosestSet,
    candidates: ClosestCandidates,

    iteration_count: usize,

    done_on_eligible_result: bool,
    lookup_done: bool,
}

impl LookupTaskData {
    const MAX_ITERATIONS: usize = 3*KBucket::MAX_ENTRIES;

    pub(crate) fn new(target: Id, done_on_eligible_result: bool) -> Self {
        Self {
            closest: ClosestSet::new(
                target.clone(),
                KBucket::MAX_ENTRIES
            ),
            candidates: ClosestCandidates::new(
                target.clone(),
                3 * KBucket::MAX_ENTRIES
            ),

            target,
            iteration_count: 0,
            done_on_eligible_result,
            lookup_done: false,
        }
    }
}

pub(crate) trait LookupTask {
    fn base_data(&self) -> &TaskData;
    fn dht(&self) -> Arc<Mutex<DHT>>;

    fn data(&self) -> &LookupTaskData;
    fn data_mut(&mut self) -> &mut LookupTaskData;

    fn target(&self) -> &Id {
        &self.data().target
    }

    fn candidate_size(&self) -> usize {
        self.data().candidates.size()
    }

    fn candidate_node(&self, _: &Id) -> Option<CandidateNode> {
        unimplemented!()
    }

    fn add_candidates_with_nodes(&mut self, mut nodes: Vec<NodeInfo>) {
        let dht = self.dht();
        let locked = dht.lock().unwrap();

        let mut todo: Vec<NodeInfo> = Vec::new();
        while let Some(ni) = nodes.pop() {
            let bogon = if cfg!(feature = "devp") {
                !is_any_unicast(&ni.ip())
            } else {
                is_bogon(&ni.socket_addr())
            };

            if bogon ||self.data().closest.contains(ni.id()) ||
                locked.id() == ni.id() ||
                locked.addr() == ni.socket_addr() {
                continue;
            }
            todo.push(ni);
        }

        if !todo.is_empty() {
            self.data_mut().candidates.add(todo)
        }
    }

    fn add_candidates_with_kentries(&mut self, mut entries: Vec<KBucketEntry>) {
        let dht = self.dht();
        let locked = dht.lock().unwrap();

        let mut todo: Vec<KBucketEntry> = Vec::new();
        while let Some(entry) = entries.pop() {
            let ni = entry.as_ref();
            let bogon = if cfg!(feature = "devp") {
                !is_any_unicast(&ni.ip())
            } else {
                is_bogon(&ni.socket_addr())
            };

            if bogon ||self.data().closest.contains(ni.id()) ||
                locked.id() == ni.id() ||
                locked.addr() == ni.socket_addr() {
                continue;
            }
            todo.push(entry);
        }

        if !todo.is_empty() {
            self.data_mut().candidates.add(todo)
        }
    }

    fn remove_candidate(&mut self, id: &Id) -> Option<Arc<Mutex<CandidateNode>>> {
        self.data_mut().candidates.remove(id)
    }

    fn next_candidate(&mut self) -> Option<Arc<Mutex<CandidateNode>>> {
       self.data_mut().candidates.next()
    }

    fn candidate_empty(&self) -> bool {
        self.data().candidates.size() == 0
    }

    fn add_closest(&mut self, cn: Arc<Mutex<CandidateNode>>) {
        self.data_mut().closest.add(cn)
    }

    fn closest(&self) -> &ClosestSet {
        &self.data().closest
    }

    fn iterate(&mut self) {
        self.data_mut().iteration_count += 1;
    }

    fn is_done(&self) -> bool {
        let data = self.data();
        if data.lookup_done {
            return true;
        }
        if data.iteration_count >= LookupTaskData::MAX_ITERATIONS {
            return true;
        }
        if !self.base_data().is_done() {
            return false;
        }
        if data.candidates.size() == 0 {
            return true;
        }
        data.closest.is_eligible() && data.target.three_way_compare(
            &data.closest.tail(), &data.candidates.head()
        ).is_le()
    }

    fn call_error(&mut self, call: &RpcCall) {
        let target = call.target();
        let id = target.id();

        match target {
            NodeEntry::CandidateNode(_) => self.remove_candidate(&id),
            _ => self.remove_candidate(&id)
        };
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        let target = call.target();
        let id = target.id();

        match target {
            NodeEntry::CandidateNode(cn) => {
                let reachable = cn.lock().unwrap().is_reachable();
                if reachable {
                    self.remove_candidate(&id);
                } else {
                    cn.lock().unwrap().clear_sent();
                }
            },
            _ => {self.remove_candidate(&id);}
        }
    }

    fn call_responsed(&mut self, call: &RpcCall) {
        if let Some(cn) = self.remove_candidate(&call.target_id()) {
            cn.lock().unwrap().set_replied();

            let Some(rsp) = call.rsp() else {
                return;
            };

            let token = match rsp.lock().unwrap().body() {
                Some(Body::FindNodeRsp(body)) => body.token(),
                Some(Body::FindPeerRsp(body)) => body.token(),
                Some(Body::FindValueRsp(body)) => body.token(),
                _ => return,
            };
            cn.lock().unwrap().set_token(token);
            self.add_closest(cn);
        }
    }

    fn done_on_eligible_result(&self) -> bool {
        self.data().done_on_eligible_result
    }

    fn mark_lookup_done(&mut self) {
        self.data_mut().lookup_done = true;
    }
}
