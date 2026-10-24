// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{cmp::Ordering, collections::HashMap};

use boson_api::{Id, PeerInfo};

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
        self.expected_count > 0 && self.peers.len() >= self.expected_count
    }

    pub(crate) fn add(&mut self, peers: Vec<PeerInfo>, latest: bool) -> bool {
        for peer in &peers {
            if !self.is_peer_eligible(peer) {
                return false;
            }
        }

        for peer in peers {
            let key = (peer.id().clone(), peer.fingerprint());
            if let Some(existing) = self.peers.get_mut(&key) {
                if existing.sequence_number() < peer.sequence_number() {
                    *existing = peer;
                    self.latest = latest;
                }
            } else {
                self.peers.insert(key, peer);
                self.latest = latest;
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
        all.truncate(self.expected_count);

        self.peers = all
            .into_iter()
            .map(|p| ((p.id().clone(), p.fingerprint()), p))
            .collect();
    }

    pub(crate) fn peers(&self) -> Vec<PeerInfo> {
        self.peers.values().cloned().collect()
    }

    fn is_peer_eligible(&self, peer: &PeerInfo) -> bool {
        peer.id() == &self.target
            && peer.is_valid()
            && (self.expected_seq < 0 || peer.sequence_number() >= self.expected_seq)
    }

    fn peer_order(&self, left: &PeerInfo, right: &PeerInfo) -> Ordering {
        right
            .sequence_number()
            .cmp(&left.sequence_number())
            .then_with(|| right.is_authenticated().cmp(&left.is_authenticated()))
            .then_with(|| match (left.nodeid(), right.nodeid()) {
                (Some(l), Some(r)) => self.target.three_way_compare(l, r),
                _ => Ordering::Equal,
            })
    }
}
