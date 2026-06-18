use std::{
    any::Any,
    sync::{Arc, Mutex},
    collections::VecDeque,
};
use log::{debug, error, warn};

use crate::PeerInfo;
use crate::dht::{
    dht::DHT,
    msg::msg,
    consumer::Consumer,
    task::{ClosestSet, CandidateNode,Task, TaskData}
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
            todo: Arc::new(Mutex::new(
                VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            expected_seq
        }
    }

    pub(crate) fn with_closest(&self, closest: ClosestSet) -> &Self {
        let mut loced_todo = self.todo.lock().unwrap();
        let mut entries = closest.entries();

        while let Some(cn) = entries.pop() {
            if loced_todo.len() >= MAX_TODO_ENTRIES {
                break;
            }
            loced_todo.push_back(cn);
        }

        debug!(
            "{}#{} added {} eligible nodes to announce queue",
            self.task_name(),
            self.task_id(),
            loced_todo.len()
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

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let cn = match self.todo.lock().unwrap().front() {
                Some(cn) => cn.clone(),
                _ => break,
            };

            let token = cn.lock().unwrap().token();
            if token == 0 {
                warn!("{}#{} skip announcing to {} due to missing token",
                    self.task_name(),
                    self.task_id(),
                    cn.lock().unwrap().id(),
                );
                self.todo.lock().unwrap().pop_front();
                continue;
            }

            let msg = msg::announce_peer_request(
                self.peer.clone(), token, self.expected_seq,
            );

            let cloned_todo = self.todo.clone();
            let cb = Consumer::new(move |_| {
                cloned_todo.lock().unwrap().pop_front();
            });

            if let Err(e) = self.send_call(cn.into(), msg, Some(cb)) {
                error!("Sending 'announcePeer' request error: {}", e);
             };
        }
    }

    fn is_done(&self) -> bool {
        self.todo.lock().unwrap().is_empty() &&
            self.data().is_done()
    }
}

unsafe impl Send for PeerAnnounceTask {}
unsafe impl Sync for PeerAnnounceTask {}
