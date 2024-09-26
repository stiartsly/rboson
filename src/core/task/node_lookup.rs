use std::any::Any;
use std::rc::Rc;
use std::cell::RefCell;
use log::error;

use crate::{
    Id,
    id::MAX_ID,
    Network,
    NodeInfo
};

use crate::core::{
    constants,
    rpccall::RpcCall,
    dht::DHT,
    kclosest_nodes::KClosestNodes,
};

use crate::core::msg::{
    find_node_req as req,
    find_node_rsp as rsp,
    msg::{Method, Kind, Msg},
    lookup_req::{Msg as LookupRequest},
    lookup_rsp::{Msg as LookupResponse},
};

use super::{
    task::{Task, TaskData},
    lookup_task::{LookupTask, LookupTaskData},
};

pub(crate) struct NodeLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    bootstrap : bool,
    want_token: bool,
    result_fn: Option<Box<dyn FnMut(&mut dyn Task, Option<Rc<NodeInfo>>)>>,
    listeners: Vec<Box<dyn FnMut(&mut dyn Task)>>,
}

impl NodeLookupTask {
    pub(crate) fn new(target: Rc<Id>, dht: Rc<RefCell<DHT>>) -> Self {
        Self {
            base_data: TaskData::new(dht),
            lookup_data: LookupTaskData::new(target),
            bootstrap: false,
            want_token: false,
            result_fn: Some(Box::new(|_,_|{})),
            listeners: Vec::new(),
        }
    }

    pub(crate) fn set_bootstrap(&mut self, bootstrap: bool) {
        self.bootstrap = bootstrap
    }

    pub(crate) fn set_want_token(&mut self, token: bool) {
        self.want_token = token
    }

    pub(crate) fn inject_candidates(&mut self, nodes: &[Rc<NodeInfo>]) {
        self.add_candidates(nodes)
    }

    pub(crate) fn set_result_fn<F>(&mut self, f: F)
    where F: FnMut(&mut dyn Task , Option<Rc<NodeInfo>>) + 'static {
        self.result_fn = Some(Box::new(f));
    }
}

impl LookupTask for NodeLookupTask {
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

impl Task for NodeLookupTask {
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
            // if we're bootstrapping start from the bucket that has the greatest
            // possible distance from ourselves so we discover new things along
            // the (longer) path.
            let target = match self.bootstrap {
                true => Rc::new(self.target().distance(&MAX_ID)),
                false => self.target().clone()
            };

            // delay the filling of the todo list until we actually start the task
            let mut kns = KClosestNodes::with_filter(
                target,
                Task::data(self).dht().borrow().ni(),
                Task::data(self).dht().borrow().rt(),
                constants::MAX_ENTRIES_PER_BUCKET *2,
                move |e| e.borrow().is_eligible_for_nodes_list()
            );
            kns.fill(false);
            kns.as_nodes()
        };

        self.add_candidates(&nodes);
    }

    fn update(&mut self) {
        while self.can_request() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next.clone(),
                None => break
            };

            let msg = Rc::new(RefCell::new({
                let mut msg = Box::new(req::Message::new());
                msg.with_target(self.target());
                msg.with_want4(true);
                msg.with_want6(false);
                msg.with_want_token(self.want_token);
                msg as Box<dyn Msg>
            }));

            let next = next.clone();
            let ni = next.borrow().ni();

            self.send_call(ni, msg, Box::new(move|_| {
                next.borrow_mut().set_sent();
            })).map_err(|e| {
                error!("Error on sending 'findNode' message: {}", e);
            }).ok();
        }
    }

    fn call_responsed(&mut self, call: &RpcCall, msg: Rc<RefCell<Box<dyn Msg>>>) {
        if !call.matches_id()||
            msg.borrow().kind() != Kind::Response ||
            msg.borrow().method() != Method::FindNode {
            return;
        }

        let borrowed = msg.borrow();
        let msg = match borrowed.as_any().downcast_ref::<rsp::Message>() {
            Some(v) => v,
            None => return
        };

        LookupTask::call_responsed(self, call, msg);

        let network = self.dht().borrow().network();
        let nodes = match network {
            Network::IPv4 => LookupResponse::nodes4(msg),
            Network::IPv6 => LookupResponse::nodes6(msg)
        };

        if let Some(nodes) = nodes {
            if !nodes.is_empty() {
                self.add_candidates(nodes);
            }
            for item in nodes.iter() {
                if *self.target() == *item.id() {
                    let mut cb = self.result_fn.take();
                    (cb.as_mut().unwrap())(self, Some(item.clone()));
                    self.result_fn = cb;
                }
            }
        };
    }

    fn call_error(&mut self, call: &RpcCall) {
        LookupTask::call_error(self, call);
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        LookupTask::call_timeout(self, call);
    }

    fn is_done(&self) -> bool {
        self.base_data.is_done() || LookupTask::is_done(self) // TODO: || ->> &&
    }
}
