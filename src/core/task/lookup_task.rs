use std::rc::Rc;
use std::cell::RefCell;

use crate::{
    is_bogon_addr,
    Id,
    NodeInfo
};

use crate::core::{
    constants,
    node_info::Reachable,
    rpccall::RpcCall,
    dht::DHT,
};

use crate::core::msg::lookup_rsp::{
    Msg as LookupResponse,
};

use super::{
    closest_set::ClosestSet,
    closest_candidates::ClosestCandidates,
    candidate_node::CandidateNode,
};

pub(crate) struct LookupTaskData {
    target: Rc<Id>,
    closest_set: Rc<RefCell<ClosestSet>>,
    closest_candidates: ClosestCandidates,
}

impl LookupTaskData {
    pub(crate) fn new(target: Rc<Id>) -> Self {
        Self {
            target: target.clone(),
            closest_set: Rc::new(RefCell::new(ClosestSet::new(
                target.clone(),
                constants::MAX_ENTRIES_PER_BUCKET
            ))),
            closest_candidates: ClosestCandidates::new(
                target, 3 * constants::MAX_ENTRIES_PER_BUCKET,
            )
        }
    }
}

pub(crate) trait LookupTask {
    fn data(&self) -> &LookupTaskData;
    fn data_mut(&mut self) -> &mut LookupTaskData;
    fn dht(&self) -> Rc<RefCell<DHT>>;

    fn target(&self) -> Rc<Id> {
        self.data().target.clone()
    }

    fn closest_set(&self) -> Rc<RefCell<ClosestSet>> {
        self.data().closest_set.clone()
    }

    fn add_candidates(&mut self, nodes: &[Rc<NodeInfo>]) {
        let mut candidates = Vec::new();
        let dht = self.dht();

        for item in nodes.iter() {
            if is_bogon_addr!(item.socket_addr()) ||
                dht.borrow().id() == item.id() ||
                dht.borrow().addr() == item.socket_addr() ||
                self.data().closest_set.borrow().contains(item.id()) {
                continue;
            }
            candidates.push(item.clone());
        }

        if !candidates.is_empty() {
            self.data_mut().closest_candidates.add(&candidates)
        }
    }

    fn remove_candidate(&mut self, id: &Id) -> Option<Rc<RefCell<CandidateNode>>> {
        self.data_mut().closest_candidates.remove(id)
    }

    fn next_candidate(&mut self) -> Option<Rc<RefCell<CandidateNode>>> {
       self.data_mut().closest_candidates.next()
    }

    fn add_closest(&mut self, candidate_node: Rc<RefCell<CandidateNode>>) {
        self.data_mut().closest_set.borrow_mut().add(candidate_node)
    }

    fn is_done(&self) -> bool {
        let data = self.data();
        data.closest_candidates.size() == 0 ||
            (data.closest_set.borrow().is_eligible() &&
                data.target.three_way_compare(
                    &data.closest_set.borrow().tail(), &data.closest_candidates.head()).is_le())
    }

    fn call_responsed(&mut self, call: &RpcCall, msg: &dyn LookupResponse) {
        if let Some(cn) = self.remove_candidate(call.target_id()) {
            cn.borrow_mut().set_replied();
            cn.borrow_mut().set_token(msg.token());
            self.add_closest(cn);
        }
    }

    fn call_error(&mut self, call: &RpcCall) {
        _ = self.remove_candidate(call.target_id())
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        let mut cn = Box::new(CandidateNode::new(call.target(), false));
        if cn.unreachable() {
            self.remove_candidate(cn.id());
            return;
        }
        // Clear the sent time-stamp and make it available again for the next retry
        cn.clear_sent()
    }
}
