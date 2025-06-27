use std::any::Any;
use std::collections::LinkedList;
use std::rc::Rc;
use std::cell::RefCell;
use log::error;

use crate::PeerInfo;
use crate::dht::{
    dht::DHT
};

use crate::dht::msg::{
    msg::Msg,
    announce_peer_req as req,
};

use super::{
    closest_set::ClosestSet,
    candidate_node::CandidateNode,
    task::{Task, TaskData},
};

pub(crate) struct PeerAnnounceTask {
    base_data: TaskData,

    listeners: Vec<Box<dyn FnMut(&mut dyn Task)>>,

    todo: Rc<RefCell<LinkedList<Rc<RefCell<CandidateNode>>>>>,
    peer: Rc<PeerInfo>,
}

impl PeerAnnounceTask {
    pub(crate) fn new(
        dht: Rc<RefCell<DHT>>,
        closest: Rc<RefCell<ClosestSet>>,
        peer: Rc<PeerInfo>
    ) -> Self {
        let todo: LinkedList<_> = closest.borrow()
            .entries()
            .iter()
            .cloned()
            .collect();

        Self {
            base_data: TaskData::new(dht),
            listeners: Vec::new(),
            peer,
            todo: Rc::new(RefCell::new(todo)),
        }
    }
}

impl Task for PeerAnnounceTask {
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

    fn update(&mut self) {
        while self.can_request() {
            let cn = match self.todo.borrow().front() {
                Some(cn) => cn.clone(),
                None => break,
            };

            let msg = Rc::new(RefCell::new({
                let mut msg = Box::new(req::Message::new());
                msg.with_peer(self.peer.clone());
                msg.with_token(cn.borrow().token());
                msg as Box<dyn Msg>
            }));

            let todo = self.todo.clone();
            let ni = cn.borrow().ni();

            self.send_call(ni, msg, Box::new(move|_| {
                todo.borrow_mut().pop_front();
            })).map_err(|e| {
               error!("Error on sending 'announcePeer' message: {}", e);
            }).ok();
        }
    }
}
