use std::{
    sync::{Arc, Mutex},
    collections::VecDeque,
};
use log::{debug, error};

use crate::PeerInfo;
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    msg::msg::Message,
    rpc::node_entry::NodeEntry,
    task::{
        closest_set::ClosestSet,
        candidate_node::CandidateNode,
        task::{Task, TaskData},
    }
};

pub(crate) struct PeerAnnounceTask {
    base_data: TaskData,

    todo: Arc<Mutex<VecDeque<Arc<Mutex<CandidateNode>>>>>,
    peer: PeerInfo,
    expected_seq: i32,

    dht: Arc<Mutex<DHT>>,
}

const MAX_TODO_ENTRIES: usize = 24;

impl PeerAnnounceTask {
    pub(crate) fn new(
        dht: Arc<Mutex<DHT>>,
        peer: PeerInfo,
        expected_seq: i32
    ) -> Self {
        Self {
            base_data: TaskData::new(),
            peer,
            todo: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            expected_seq,
            dht,
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

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn iterate(&mut self) {
        println!("can_dorequest: {}", self.can_dorequest());
        while self.can_dorequest() {
            println!("Iterating PeerAnnounceTask: todo.len()={}", self.todo.lock().unwrap().len());
            let cn = match self.todo.lock().unwrap().front() {
                Some(cn) => cn.clone(),
                None => break,
            };

            let token = cn.lock().unwrap().token();
            if token == 0 {
                self.todo.lock().unwrap().pop_front();
                continue;
            }

            let msg = Message::announce_peer_req(
                self.peer.clone(),
                token,
                self.expected_seq,
            );

            let todo = self.todo.clone();
            let entry= NodeEntry::from_candidate(cn);
            let msg  = Arc::new(Mutex::new(msg));
            let handler = Consumer::new(move || {
                todo.lock().unwrap().pop_front();
            });
             let _ = self.send_call(entry, msg, Some(handler)).map_err(|e| {
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
