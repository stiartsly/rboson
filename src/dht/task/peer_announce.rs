use std::sync::{Arc, Mutex};
use std::collections::LinkedList;
use log::error;

use crate::PeerInfo;
use crate::dht::{
    dht::DHT,
    msg::msg::Message,
    node_entry::NodeEntry,
};

use super::{
    closest_set::ClosestSet,
    candidate_node::CandidateNode,
    task::{Task, TaskData},
};

pub(crate) struct PeerAnnounceTask {
    base_data: TaskData,

    todo: Arc<Mutex<LinkedList<Arc<Mutex<CandidateNode>>>>>,
    peer: PeerInfo,
    expected_seq: i32,
}

impl PeerAnnounceTask {
    pub(crate) fn new(dht: Arc<Mutex<DHT>>, peer: PeerInfo, expected_seq: i32) -> Self {
        Self {
            base_data: TaskData::new(dht),
            peer,
            todo: Arc::new(Mutex::new(LinkedList::new())),
            expected_seq,
        }
    }

    pub(crate) fn with_closest(&mut self, closest: ClosestSet) -> &mut Self {
        let mut locked = self.todo.lock().unwrap();
        let mut entries = closest.entries();
        while let Some(cn) = entries.pop() {
            locked.push_back(cn);
        }
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

            let msg = Arc::new(Mutex::new(Message::announce_peer_req(
                self.peer.clone(),
                token,
                self.expected_seq,
            )));

            let todo = self.todo.clone();
            let ne   = NodeEntry::from_candidate(cn);

            self.send_call(ne, msg, Box::new(move|_| {
                todo.lock().unwrap().pop_front();
            })).map_err(|e| {
               error!("Error on sending 'announcePeer' message: {}", e);
            }).ok();
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() && Task::is_done(self)
    }
}

unsafe impl Send for PeerAnnounceTask {}
unsafe impl Sync for PeerAnnounceTask {}
