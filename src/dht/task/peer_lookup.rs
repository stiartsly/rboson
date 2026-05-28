use std::{
    any::Any,
    sync::{Arc, Mutex}
};
use log::{debug, error, warn};

use crate::{Id, PeerInfo};
use crate::dht::{
    dht::DHT,
    consumer::Consumer,
    eligible_peers::EligiblePeers,
    rpc::{
        rpccall::RpcCall,
        rpc_target::Target,
    },
    routing::{
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
        kclosest_nodes::KClosestNodes,
    },
    msg::{
        msg::{Body, Message},
        lookup_rsp::LookupResponse
    },
    task::{
        task::{Task, TaskData},
        lookup_task::{LookupTask, LookupTaskData},
    },
};

pub(crate) struct PeerLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    result: EligiblePeers,
    dht: Arc<Mutex<DHT>>
}

impl PeerLookupTask {
    pub(crate) fn new(
        dht: Arc<Mutex<DHT>>,
        target: Id,
        expected_seq: i32,
        expected_count: usize,
        done_on_eligible_result: bool
    ) -> Self {
        Self {
            base_data: TaskData::new(),
            lookup_data: LookupTaskData::new(target, done_on_eligible_result),
            result: EligiblePeers::new(target, expected_seq, expected_count),
            dht,
        }
    }

    pub(crate) fn result(&self) -> Vec<PeerInfo> {
        self.result.peers().to_vec()
    }
}

impl LookupTask for PeerLookupTask {
    fn base_data(&self) -> &TaskData {
        &self.base_data
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    fn data(&self) -> &LookupTaskData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookupTaskData {
        &mut self.lookup_data
    }
}

impl Task for PeerLookupTask {
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

    fn prepare(&mut self) {
        let rt = self.dht.lock().unwrap().rt();
        let entries:Vec<KBucketEntry> = {
            let mut kns = KClosestNodes::new(
                rt,
                self.target().clone(),
                KBucket::MAX_ENTRIES *3
            );
            kns.set_filter(|v| v.eligible_for_local_lookup());
            kns.fill();
            kns.into()
        };

        debug!("{}#{} initialized {} candidates for target {}",
            self.task_name(),
            self.task_id(),
            entries.len(),
            self.target()
        );
        self.add_candidates_with_kentries(entries);
    }

    fn iterate(&mut self) {
        LookupTask::iterate(self);

        while self.can_dorequest() {
            let next = match LookupTask::next_candidate(self) {
                Some(next) => next.clone(),
                None => break,
            };

            let target = Target::from_candidate(next.clone());
            let network = self.dht.lock().unwrap().network();
            let msg = Message::find_peer_req(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.result.expected_seq(),
                self.result.expected_count() as i32,
            );

            let handler = Consumer::new(move || {
                next.lock().unwrap().set_sent();
            });
             let _ = self.send_call(target, msg, Some(handler)).map_err(|e| {
                error!("Sending 'findPeer' request error: {}", e);
             });
        }
    }

    fn call_responded(&mut self, call: &RpcCall) {
        LookupTask::call_responded(self, call);

        if call.id_mismatched() {
            return;
        }
        let msg = call.rsp().expect("panic: no response set");
        let Body::FindPeerRsp(body) = msg.body().expect("no message body") else {
            return;
        };

        if let Some(peers) = body.peers() {
            if peers.is_empty() {
                warn!("{}#{} received empty peers from {}, ignoring",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            }

            if !self.result.add(peers.to_vec(), false) {
                warn!(
                    "{}#{} dropping peer response from {} due to ineligible peer data",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            }

            debug!("{}#{} received {} peers from response by {}",
                self.task_name(),
                self.task_id(),
                peers.len(),
                call.target_id()
            );

            if self.result.reached_capacity() {
                if LookupTask::data(self).done_on_eligible_result() {
                    LookupTask::data_mut(self).mark_lookup_done();
                }
                self.result.prune();;
            }
        } else {
            let network = self.dht.lock().unwrap().network();
            let nodes = body.nodes(network);

            let Some(nodes) = nodes.filter(|v| !v.is_empty()) else {
                warn!("{}#{} received empty nodes list from {}, ignoring",
                    self.task_name(),
                    self.task_id(),
                    call.target_id()
                );
                return;
            };

            self.add_candidates_with_nodes(nodes.to_vec());

            debug!("{}#{} added {} candidates from response by {}",
                self.task_name(),
                self.task_id(),
                nodes.len(),
                call.target_id()
            );
        }
    }

    fn call_error(&mut self, call: &RpcCall) {
        LookupTask::call_error(self, call);
    }

    fn call_timeout(&mut self, call: &RpcCall) {
        LookupTask::call_timeout(self, call);
    }

    fn is_done(&self) -> bool {
        LookupTask::is_done(self)
    }
}

unsafe impl Send for PeerLookupTask {}
unsafe impl Sync for PeerLookupTask {}
