use std::{
    any::Any,
    rc::Rc,
    cell::RefCell,
    collections::VecDeque,
};
use crate::PeerInfo;
use crate::dht::{
    dht::DHT,
    msg::msg,
    handler::Handler,
    task::{ClosestSet, CandidateNode,Task, TaskData}
};

pub(crate) struct PeerAnnounceTask {
    base_data: TaskData,

    todo: Rc<RefCell<VecDeque<Rc<RefCell<CandidateNode>>>>>,
    peer: PeerInfo,
    expected_seq: i32,

    dht: Rc<RefCell<DHT>>,
}

const MAX_TODO_ENTRIES: usize = 24;

impl PeerAnnounceTask {
    pub(crate) fn new(
        dht: Rc<RefCell<DHT>>,
        peer: PeerInfo,
        expected_seq: i32
    ) -> Self {
        Self {
            dht,
            base_data: TaskData::new(),
            peer,
            todo: Rc::new(RefCell::new(
                VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            expected_seq
        }
    }

    pub(crate) fn with_closest(&self, closest: ClosestSet) -> &Self {
        let mut borrowed_todo = self.todo.borrow_mut();
        let mut entries = closest.entries();

        while let Some(cn) = entries.pop() {
            if borrowed_todo.len() >= MAX_TODO_ENTRIES {
                break;
            }
            borrowed_todo.push_back(cn);
        }

        log::debug!(
            "{}#{} added {} eligible nodes to announce queue",
            self.task_name(),
            self.task_id(),
            borrowed_todo.len()
        );
        self
    }
}

impl Task for PeerAnnounceTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn as_task(&self) -> &dyn Task {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dht(&self) -> Rc<RefCell<DHT>> {
        self.dht.clone()
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let cn = match self.todo.borrow().front() {
                Some(cn) => cn.clone(),
                _ => break,
            };

            let token = cn.borrow().token();
            if token == 0 {
                log::warn!("{}#{} skip announcing to {} due to missing token",
                    self.task_name(),
                    self.task_id(),
                    cn.borrow().id(),
                );
                self.todo.borrow_mut().pop_front();
                continue;
            }

            let msg = msg::announce_peer_request(
                self.peer.clone(), token, self.expected_seq,
            );

            let cloned_todo = self.todo.clone();
            let cb = Handler::new(move |_| {
                cloned_todo.borrow_mut().pop_front();
            });
            self.send_call(cn.into(), msg, Some(cb));
        }
    }

    fn is_done(&self) -> bool {
        self.todo.borrow().is_empty() &&
            self.data().is_done()
    }
}
