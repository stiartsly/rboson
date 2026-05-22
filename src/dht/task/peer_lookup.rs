use std::sync::{Arc, Mutex};
use log::{debug, error, warn};

use crate::{Id, Network};
use crate::dht::{
    dht::DHT,
    rpccall::RpcCall,
    node_entry::NodeEntry,
    eligible_peers::EligiblePeers,
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
        task::{Task, TaskData, TaskResult},
        lookup_task::{LookupTask, LookupTaskData},
    },
};

pub(crate) struct PeerLookupTask {
    base_data: TaskData,
    lookup_data: LookupTaskData,

    expected_seq: i32,
    expected_count: usize,

    result: EligiblePeers,
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
            base_data: TaskData::new(dht),
            lookup_data: LookupTaskData::new(target.clone(), done_on_eligible_result),
            expected_seq,
            expected_count,
            result: EligiblePeers::new(target, expected_seq, expected_count),
        }
    }
}

impl LookupTask for PeerLookupTask {
    fn base_data(&self) -> &TaskData {
        &Task::data(self)
    }

    fn dht(&self) -> Arc<Mutex<DHT>> {
        Task::data(self).dht()
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

    fn result(&self) -> Option<TaskResult> {
        Some(TaskResult::PeerInfo(self.result.peers().to_vec()))
    }

    fn prepare(&mut self) {
        let entries:Vec<KBucketEntry> = {
            let mut kns = KClosestNodes::new(
                self.dht().lock().unwrap().rt(),
                self.target().clone(),
                KBucket::MAX_ENTRIES *3
            );
            kns.set_filter(|v| v.eligible_for_local_lookup());
            kns.fill();
            kns.into()
        };

        debug!("{}#{} initialized {} candidates for target {}",
            self.name(),
            self.id(),
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

            let entry = NodeEntry::from_candidate(next.clone());
            let network = self.dht().lock().unwrap().network();
            let msg = Arc::new(Mutex::new(Message::find_peer_req(
                self.target().clone(),
                network.is_ipv4(),
                network.is_ipv6(),
                self.expected_seq,
                self.expected_count as i32,
            )));

            _ = self.send_call(entry, msg, Box::new(move |_| {
                next.lock().unwrap().set_sent();
            })).map_err(|e| {
                error!("Error on sending 'find_peer' request: {}", e);
            });
        }
    }

    fn call_responsed(&mut self, call: &RpcCall) {
        LookupTask::call_responsed(self, call);

        if call.id_mismatched() {
            return;
        }

        let Some(msg) = call.rsp() else {
            return;
        };
        let locked_msg = msg.lock().unwrap();
        let Some(Body::FindPeerRsp(body)) = locked_msg.body() else {
            warn!("{}#{} ignoring non LookupPeer response from {}",
                self.name(),
                self.id(),
                call.target_id()
            );
            return;
        };

        if let Some(peers) = body.peers() {
            if peers.is_empty() {
                warn!("{}#{} received empty peer list from {}, ignoring",
                    self.name(),
                    self.id(),
                    call.target_id()
                );
                return;
            }

            if self.result.add(peers.to_vec(), true) {
                warn!(
                    "{}#{} dropping peer response from {} due to ineligible peer data",
                    self.name(),
                    self.id(),
                    call.target_id()
                );
                return;
            }

            debug!("{}#{} received {} additional peers from {} for target {}",
                self.name(),
                self.id(),
                peers.len(),
                call.target_id(),
                self.target()
            );

            if self.result.reached_capacity() {
                if LookupTask::done_on_eligible_result(self) {
                    LookupTask::mark_lookup_done(self);
                }
                self.result.prune();
            }
        } else {
            let network = self.dht().lock().unwrap().network();
            let nodes = match network {
                Network::IPv4 => body.nodes4(),
                Network::IPv6 => body.nodes6()
            };

            let Some(nodes) = nodes.filter(|v| !v.is_empty()) else {
                warn!("{}#{} received empty nodes list from {}, ignoring",
                    self.name(),
                    self.id(),
                    call.target_id()
                );
                return;
            };

            self.add_candidates_with_nodes(nodes.to_vec());

            debug!("{}#{} added {} additional candidates from {} for target {}",
                self.name(),
                self.id(),
                nodes.len(),
                call.target_id(),
                self.target()
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
