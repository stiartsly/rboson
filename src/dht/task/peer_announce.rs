use std::{
    any::Any,
    sync::{Arc, Mutex},
    collections::VecDeque,
};
use log::{debug, error};

use crate::PeerInfo;
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    msg::msg,
    rpc::Target,
    task::{
        ClosestSet,
        CandidateNode,
        Task, TaskData,
    }
};

pub(crate) struct PeerAnnounceTask {
    base_data: TaskData,

    todo: Arc<Mutex<VecDeque<Arc<Mutex<CandidateNode>>>>>,
    peer: PeerInfo,
    expected_seq: i32,

    dht: Arc<Mutex<DHT>>
}

const MAX_TODO_ENTRIES: usize = 24;

impl PeerAnnounceTask {
    pub(crate) fn new(
        dht: Arc<Mutex<DHT>>,
        peer: PeerInfo,
        expected_seq: i32
    ) -> Self {
        Self {
            dht,
            base_data: TaskData::new(),
            peer,
            todo: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            expected_seq
        }
    }

    pub(crate) fn with_closest(&mut self, closest: ClosestSet) -> &mut Self {
        let mut locked = self.todo.lock().unwrap();
        let mut entries = closest.entries();

        while let Some(cn) = entries.pop() {
            if locked.len() >= MAX_TODO_ENTRIES {
                break;
            }
            locked.push_back(cn);
        }
        debug!(
            "{}#{} added {} eligible nodes to announce queue",
            self.task_name(),
            self.task_id(),
            locked.len()
        );
        drop(locked);
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let cn = match self.todo.lock().unwrap().front() {
                Some(cn) => cn.clone(),
                None => break,
            };

            let token = cn.lock().unwrap().token();
            if token == 0 {
                self.todo.lock().unwrap().pop_front();
                continue;
            }

            let msg = msg::announce_peer_request(
                self.peer.clone(),
                token,
                self.expected_seq,
            );

            let todo = self.todo.clone();
            let target = Target::from_candidate(cn);
            let handler = Consumer::new(move |_| {
                todo.lock().unwrap().pop_front();
            });
             let _ = self.send_call(target, msg, Some(handler)).map_err(|e| {
                error!("Sending 'announcePeer' request error: {}", e);
             });
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() && self.data().is_done()
    }
}

unsafe impl Send for PeerAnnounceTask {}
unsafe impl Sync for PeerAnnounceTask {}
