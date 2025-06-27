use std::any::Any;
use std::rc::Rc;
use std::cell::RefCell;
use log::error;

use crate::{
    Id,
    Network,
    PeerInfo
};

use crate::dht::{
    constants,
    dht::DHT,
    rpccall::RpcCall,
    kclosest_nodes::KClosestNodes,
};

use crate::dht::msg::{
    find_peer_req as req,
    find_peer_rsp as rsp,
    msg::{Method, Kind, Msg},
    lookup_req::{Msg as LookupRequest},
    lookup_rsp::{Msg as LookupResponse}
};

use super::{
    task::{Task, TaskData},
    lookup_task::{LookupTask, LookupTaskData},
};

pub(crate) struct PeerLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    result_fn: Box<dyn FnMut(Rc<RefCell<Box<dyn Task>>>, Vec<PeerInfo>)>,
    listeners: Vec<Box<dyn FnMut(&mut dyn Task)>>,
}

impl PeerLookupTask {
    pub(crate) fn new(dht: Rc<RefCell<DHT>>, target: Rc<Id>) -> Self {
        Self {
            base_data: TaskData::new(dht),
            lookup_data: LookupTaskData::new(target),
            result_fn: Box::new(|_,_|{}),
            listeners: Vec::new(),
        }
    }

    pub(crate) fn set_result_fn<F>(&mut self, f: F)
    where F: FnMut(Rc<RefCell<Box<dyn Task>>>, Vec<PeerInfo>) + 'static
    {
        self.result_fn = Box::new(f);
    }
}

impl LookupTask for PeerLookupTask {
    fn data(&self) -> &LookupTaskData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookupTaskData {
        &mut self.lookup_data
    }

    fn dht(&self) -> Rc<RefCell<DHT>> {
        Task::data(self).dht()
    }
}

impl Task for PeerLookupTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn add_listener(&mut self, cb: Box<dyn FnMut(&mut dyn Task)>) {
        self.listeners.push(cb);

    }

    fn notify_completion(&mut self) {
        while let Some(mut cb) = self.listeners.pop() {
            cb(self)
        }
    }

    fn prepare(&mut self) {
        let nodes = {
            let mut kns = KClosestNodes::with_filter(
                LookupTask::target(self),
                Task::data(self).dht().borrow().ni(),
                Task::data(self).dht().borrow().rt(),
                constants::MAX_ENTRIES_PER_BUCKET *2,
                move |_| true
            );
            kns.fill(false);
            kns.as_nodes()
        };
        self.add_candidates(&nodes);
    }

    fn update(&mut self) {
        while self.can_request() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next,
                None => break,
            };

            let msg = Rc::new(RefCell::new({
                let mut msg = Box::new(req::Message::new());
                msg.with_target(self.target());
                msg.with_want4(self.dht().borrow().network() == Network::IPv4);
                msg.with_want6(self.dht().borrow().network() == Network::IPv6);
                msg as Box<dyn Msg>
            }));

            let next = next.clone();
            let ni = next.borrow().ni();

            self.send_call(ni, msg, Box::new(move|_| {
                next.borrow_mut().set_sent();
            })).map_err(|e| {
               error!("Error on sending 'find_peer_req' message: {}", e);
            }).ok();
        }
    }

    fn call_responsed(&mut self, call: &RpcCall, msg: Rc<RefCell<Box<dyn Msg>>>) {
        if !call.matches_id()||
            msg.borrow().kind() != Kind::Response ||
            msg.borrow().method() != Method::FindPeer {
            return;
        }

        let borrowed = msg.borrow();
        let rsp = match borrowed.as_any().downcast_ref::<rsp::Message>() {
            Some(v) => v,
            None => return
        };

        LookupTask::call_responsed(self, call, rsp);

        for peer in rsp.peers() {
            if !peer.is_valid() {
                error!("Response includes an invalid peer, signature mismatched.");
                return; // ignored.
            }
        }

        if rsp.peers().is_empty() {
            let network = self.dht().borrow().network();
            let nodes = match network {
                Network::IPv4 => LookupResponse::nodes4(rsp),
                Network::IPv6 => LookupResponse::nodes6(rsp)
            };

            if let Some(nodes) = nodes {
                if !nodes.is_empty() {
                    self.add_candidates(nodes);
                }
            };
        }

        (self.result_fn)(self.base_data.task(), rsp.peers().to_vec());
    }

    fn call_error(&mut self, call: &RpcCall) {
        LookupTask::call_error(self, call)
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        LookupTask::call_timeout(self, call)
    }
}
