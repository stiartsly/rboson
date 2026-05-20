
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::{Id, PeerInfo};

#[derive(Clone, Default)]
pub(crate) struct EligiblePeers {
    target: Id,
    expected_sequence_number: i32,
    expected_count: usize,
    peers: HashMap<(Id, u64), PeerInfo>,
}

impl EligiblePeers {
    pub(crate) fn new(target: Id, expected_sequence_number: i32, expected_count: usize) -> Self {
        Self {
            target,
            expected_sequence_number,
            expected_count,
            peers: HashMap::new(),
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.peers.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub(crate) fn reached_capacity(&self) -> bool {
        self.expected_count > 0 && self.peers.len() >= self.expected_count
    }

    pub(crate) fn add(&mut self, peers: Vec<PeerInfo>, need_update: bool) -> bool {
        if peers.iter().any(|peer| !self.is_peer_eligible(peer)) {
            return false;
        }

        for peer in peers {
            let key = (peer.id().clone(), peer.fingerprint());
            self.peers
                .entry(key)
                .and_modify(|current| {
                    if current.sequence_number() < peer.sequence_number() {
                        *current = peer.clone();
                    }
                })
                .or_insert_with(|| peer.clone());
        }

        true
    }

    pub(crate) fn needs_update(&self) -> bool {
        false
    }

    pub(crate) fn prune(&mut self) {
        if !self.reached_capacity() {
            return;
        }

        let mut peers = self.peers();
        let mut to_remove = peers.split_off(self.expected_count as usize);

        for peer in to_remove.drain(..) {
            self.peers.remove(&(peer.id().clone(), peer.fingerprint()));
        }
    }

    pub(crate) fn peers(&self) -> Vec<PeerInfo> {
        let mut peers: Vec<_> = self.peers.values().cloned().collect();
        peers.sort_by(|left, right| self.peer_order(left, right));
        peers
    }

    fn is_peer_eligible(&self, peer: &PeerInfo) -> bool {
        peer.id() == &self.target
            && (self.expected_sequence_number < 0
                || peer.sequence_number() >= self.expected_sequence_number)
            && peer.is_valid()
    }

    fn peer_order(&self, left: &PeerInfo, right: &PeerInfo) -> Ordering {
        let by_sequence = right.sequence_number().cmp(&left.sequence_number());
        if by_sequence != Ordering::Equal {
            return by_sequence;
        }

        let by_auth = right.is_authenticated().cmp(&left.is_authenticated());
        if by_auth != Ordering::Equal {
            return by_auth;
        }

        match (left.nodeid(), right.nodeid()) {
            (Some(left_node), Some(right_node)) => self.target.three_way_compare(left_node, right_node),
            _ => Ordering::Equal,
        }
    }
}