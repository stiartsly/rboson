use std::{
    cmp::Ordering,
    collections::HashMap,
};

use crate::{Id, PeerInfo};
pub(crate) struct EligiblePeers {
    target: Id,
    expected_seq: i32,
    expected_count: usize,
    peers: HashMap<(Id, u64), PeerInfo>,

    latest: bool,
}

impl EligiblePeers {
    pub(crate) fn new(target: Id, expected_seq: i32, expected_count: usize) -> Self {
        Self {
            target,
            expected_seq,
            expected_count,
            peers: HashMap::new(),
            latest: false,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn expected_count(&self) -> usize {
        self.expected_count
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub(crate) fn reached_capacity(&self) -> bool {
        self.expected_count > 0 &&
            self.peers.len() >= self.expected_count
    }

    pub(crate) fn add(&mut self, mut peers: Vec<PeerInfo>, latest: bool) -> bool {
        for peer in &peers {
            if !self.is_peer_eligible(peer) {
                return false;
            }
        }

        while let Some(peer) = peers.pop() {
            let key = (peer.id().clone(), peer.fingerprint());
            if let Some(existing) = self.peers.get_mut(&key) {
                if existing.sequence_number() < peer.sequence_number() {
                    *existing = peer;
                    self.latest = latest;
                }
            } else {
                self.latest = latest;
                self.peers.insert(key, peer);
            }
        }
        true
    }

    pub(crate) fn is_latest(&self) -> bool {
        self.latest
    }

    pub(crate) fn prune(&mut self) {
        if !self.reached_capacity() {
            return;
        }

        let mut all: Vec<PeerInfo> = self.peers.values().cloned().collect();
        all.sort_by(|l, r| self.peer_order(l, r));

        for peer in all.into_iter().skip(self.expected_count) {
            self.peers.remove(&(peer.id().clone(), peer.fingerprint()));
        }
    }

    pub(crate) fn peers(&self) -> Vec<PeerInfo> {
        self.peers.values().cloned().collect()
    }

    fn is_peer_eligible(&self, peer: &PeerInfo) -> bool {
        peer.id() == &self.target
            && peer.is_valid()
            && (self.expected_seq < 0
                || peer.sequence_number() >= self.expected_seq)

    }

    fn peer_order(&self, left: &PeerInfo, right: &PeerInfo) -> Ordering {
        let by_seq = right.sequence_number().cmp(&left.sequence_number());
        if by_seq != Ordering::Equal {
            return by_seq ;
        }

        let by_auth = right.is_authenticated().cmp(&left.is_authenticated());
        if by_auth != Ordering::Equal {
            return by_auth;
        }

        match (left.nodeid(), right.nodeid()) {
            (Some(l), Some(r)) => self.target.three_way_compare(l, r),
            _ => Ordering::Equal,
        }
    }
}