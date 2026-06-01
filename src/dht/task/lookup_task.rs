use std::sync::{Arc, Weak, Mutex};

use crate::{Id, NodeInfo};
use crate::dht::{
    dht::DHT,
    utils::{is_any_unicast, is_bogon},
    rpc::{Target, Reachability, RpcCall},
    msg::{Body,LookupResponse},
    routing::{ KBucket, KBucketEntry},
    task::{
        ClosestSet,
        ClosestCandidates,
        CandidateNode,
        TaskData,
    }
};

const MAX_ITERATIONS: usize = 3*KBucket::MAX_ENTRIES;

pub(crate) struct LookupTaskData {
    target: Id,
    closest: ClosestSet,
    candidates: ClosestCandidates,

    iteration_count: usize,

    done_on_eligible_result: bool,
    done_on_lookup: bool,
}

impl LookupTaskData {
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
            done_on_lookup: false,
        }
    }

    pub(crate) fn done_on_eligible_result(&self) -> bool {
        self.done_on_eligible_result
    }

    pub(crate) fn done_lookup(&mut self) {
        self.done_on_lookup = true;
    }
}

pub(crate) trait LookupTask {
    fn base_data(&self) -> &TaskData;
    fn dht(&self) -> Weak<Mutex<DHT>>;

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
        let strong_dht = match self.dht().upgrade() {
            Some(dht) => dht,
            None => return,
        };
        let locked_dht = strong_dht.lock().unwrap();
        let local_id   = locked_dht.id().clone();
        let local_addr = locked_dht.addr().clone();
        drop(locked_dht);

        let mut todo: Vec<NodeInfo> = Vec::new();
        while let Some(ni) = nodes.pop() {
            let bogon = if cfg!(feature = "devp") {
                !is_any_unicast(&ni.ip())
            } else {
                is_bogon(&ni.socket_addr())
            };

            if bogon ||self.data().closest.contains(ni.id()) ||
                &local_id == ni.id() ||
                &local_addr == ni.socket_addr() {
                continue;
            }
            todo.push(ni);
        }

        if !todo.is_empty() {
            self.data_mut().candidates.add(todo)
        }
    }

    fn add_candidates_with_kentries(&mut self, mut entries: Vec<KBucketEntry>) {
        let strong_dht = match self.dht().upgrade() {
            Some(dht) => dht,
            None => return,
        };
        let locked_dht = strong_dht.lock().unwrap();
        let local_id   = locked_dht.id().clone();
        let local_addr = locked_dht.addr().clone();
        drop(locked_dht);

        let mut todo: Vec<KBucketEntry> = Vec::new();
        while let Some(entry) = entries.pop() {
            let ni = entry.as_ref();
            let bogon = if cfg!(feature = "devp") {
                !is_any_unicast(&ni.ip())
            } else {
                is_bogon(&ni.socket_addr())
            };

            if bogon ||self.data().closest.contains(ni.id()) ||
                &local_id == ni.id() ||
                &local_addr == ni.socket_addr() {
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
        if data.done_on_lookup {
            return true;
        }
        if data.iteration_count >= MAX_ITERATIONS {
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
        let id = call.target().id();
        let _  = self.remove_candidate(&id);
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        let target = call.target();
        let id = target.id();

        match target {
            Target::Candidate(cn) => {
                let unreachable = cn.lock().unwrap().is_unreachable();
                if unreachable {
                    self.remove_candidate(&id);
                } else {
                    cn.lock().unwrap().clear_sent();
                }
            },
            _ => {
                let _ = self.remove_candidate(&id);
            }
        }
    }

    fn call_responded(&mut self, call: &RpcCall) {
        let target_id = call.target_id();
        let Some(cn) = self.remove_candidate(&target_id) else {
            return;
        };
        cn.lock().unwrap().set_replied();

        let Some(rsp) = call.rsp() else {
            return;
        };
        let Some(body) = rsp.body() else {
            return;
        };
        let token = match body {
            Body::FindNodeResponse(body) => body.token(),
            Body::FindPeerResponse(body) => body.token(),
            Body::FindValueResponse(body) => body.token(),
            _ => return,
        };
        cn.lock().unwrap().set_token(token);
        self.add_closest(cn);
    }
}
