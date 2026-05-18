use std::fmt;
use std::cmp::{min, max};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use ciborium::value::Value;

use crate::{
    as_secs,
    elapsed_ms,
    Id,
    NodeInfo,
    core::version,
};
use crate::dht::{
    node_entry::Reachability,
};

/**
 * Entry in a KBucket, it basically contains an IP address of a node,
 * the UDP port of the node and a node id.
 */
#[derive(Clone, Debug)]
pub(crate) struct KBucketEntry {
    ni: NodeInfo,

    created     : SystemTime,
    last_seen   : SystemTime,
    last_sent   : SystemTime,

    reachable: bool,
    failed_requests: i32,
}

impl KBucketEntry {
    const MAX_FAILURES: i32 = 5;
    const OLD_AND_STALE_FAILURES: i32 = 2;

    const OLD_AND_STALE_TIME: u64 = 15 * 60 * 1000; // 15 minutes
    const PING_BACKOFF_BASE_INTERVAL: u64 = 60 * 1000; // 1 minute

    pub(crate) fn new(id: Id, addr: SocketAddr) -> Self {
        Self {
            ni: NodeInfo::new(id, addr),
            created     : SystemTime::UNIX_EPOCH,
            last_seen   : SystemTime::UNIX_EPOCH,
            last_sent   : SystemTime::UNIX_EPOCH,
            reachable   : false,
            failed_requests: 0,
        }
    }

    pub(crate) fn set_ver(&mut self, ver: i32) {
        self.ni.set_version(ver);
    }

    pub(crate) fn id(&self) -> &Id {
        &self.ni.id()
    }

    pub(crate) fn ni(&self) -> &NodeInfo {
        &self.ni
    }

    pub(crate) fn socket_addr(&self) -> &SocketAddr {
        self.ni.socket_addr()
    }

    pub(crate) fn created_time(&self) -> &SystemTime {
        &self.created
    }

    pub(crate) fn last_seen(&self) -> &SystemTime {
        &self.last_seen
    }

    pub(crate) fn set_last_seen(&mut self, last_seen: SystemTime) {
        self.last_seen = last_seen;
    }

    pub(crate) fn last_sent(&self) -> &SystemTime {
        &self.last_sent
    }

    pub(crate) const fn failed_requests(&self) -> i32 {
        self.failed_requests
    }

    pub(crate) const fn eligible_for_nodes_list(&self) -> bool {
        // 1 timeout can occasionally happen. should be fine to hand it out
        // as long as we've verified it at least once
        self.reachable && self.failed_requests < 3
    }

    pub(crate) const fn eligible_for_local_lookup(&self) -> bool {
        // allow implicit initial ping during lookups
        // TO~DO: make this work now that we don't keep unverified entries
        // in the main bucket
        (self.reachable && self.failed_requests <= 3) ||
            self.failed_requests <= 0
    }

    fn backoff(&self) -> u64 {
        // Assertion in test case will guard the MAX_FAILURES not causing overflow
        Self::PING_BACKOFF_BASE_INTERVAL
            << min(Self::MAX_FAILURES, max(0, self.failed_requests - 1))
    }

    fn within_backoff_window(&self, _: &SystemTime) -> bool {
        self.failed_requests != 0 && elapsed_ms!(&self.last_sent) < self.backoff() as u128
    }

    fn backoff_window_end(&self) -> SystemTime {
         if self.failed_requests == 0 {
            return SystemTime::UNIX_EPOCH;
        }

        self.last_sent + Duration::from_millis(self.backoff() as u64)
    }

    /// Determines whether this node needs to be pinged to verify its reachability.
    ///
    /// The node is not pinged if it was seen recently (within 30 seconds)
    /// to allow NAT entries to expire naturally. Also respects an exponential
    /// backoff window after failed requests to reduce network traffic.
    ///
    /// Nodes with failed requests or those not seen for a long time
    /// (older than `OLD_AND_STALE_TIME`) will be pinged.
    ///
    /// # Returns
    /// `true` if the node needs a ping; `false` otherwise.

    pub(crate) fn needs_ping(&self) -> bool {
        // don't ping if recently seen to allow NAT entries to time out
        // see https://arxiv.org/pdf/1605.05606v1.pdf for numbers
        // and do exponential backoff after failures to reduce traffic
        if elapsed_ms!(self.last_seen) < 30 * 1000 ||
            self.within_backoff_window(&self.last_seen) {
            return false;
        }

        self.failed_requests != 0
            || elapsed_ms!(self.last_seen) > Self::OLD_AND_STALE_TIME as u128
    }

    fn old_and_stale(&self) -> bool {
        self.failed_requests > Self::OLD_AND_STALE_FAILURES
            && elapsed_ms!(self.last_seen) > Self::OLD_AND_STALE_TIME as u128
    }

    ///Determines if this entry can be removed from the routing table without needing replacement.
	///
    /// Entries with too many failed requests and which have not been seen since the last request
	/// sent are considered removable.
	///
	/// `true` if removable without replacement; `false` otherwise.
    ///
    pub(crate) fn removable_without_replacement(&self) -> bool {
        // some non-reachable nodes may contact us repeatedly, bumping the last seen
		// counter. they might be interesting to keep around so we can keep track of the
		// backoff interval to not waste pings on them
		// but things we haven't heard from in a while can be discarded
        let seen_since_last_sent = self.last_seen > self.last_sent;
        self.failed_requests > Self::MAX_FAILURES && !seen_since_last_sent
    }

    ///
	/// Determines if this entry needs to be replaced in the routing table.
	/// Replacement is needed if the node is unreachable with more than one failed request,
	/// if it exceeds maximum allowed timeouts, or if it is old and stale.
	///
	/// `true` if replacement is needed; `false` otherwise.
    ///
    pub(crate) fn needs_replacement(&self) -> bool {
        self.failed_requests > 1 && !self.is_reachable() ||
            self.failed_requests > Self::MAX_FAILURES ||
            self.old_and_stale()
    }

    pub(crate) fn merge(&mut self, entry: &Self) {
        if self.equals(entry) {
            return;
        }

        if entry.last_seen > self.last_seen {
            self.failed_requests = entry.failed_requests;
        }
        if entry.is_reachable() {
            self.set_reachable(true);
        }

        // TODO: average RTT.

        self.created = self.created.max(entry.created);
        self.last_seen = self.last_seen.max(entry.last_seen);
        self.last_sent = self.last_sent.max(entry.last_sent);
    }

    pub(crate) fn rtt(&self) -> u64 {
        unimplemented!()
    }

    pub(crate) fn on_request_sent(&mut self) {
        self.last_sent = SystemTime::now();
    }

    pub(crate) fn update_last_sent(&mut self, last_sent: SystemTime) {
        self.last_sent = SystemTime::max(self.last_sent, last_sent);
    }

    pub(crate) fn on_responded(&mut self, _rtt: u64) {
        self.last_seen = SystemTime::now();
        self.failed_requests = 0;
        self.reachable = true;

        // TODO: handle RTT.
    }

    pub(crate) fn on_timeout(&mut self) {
        self.failed_requests += 1;
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.ni.matches(&other.ni)
    }

    pub(crate) fn equals(&self, other: &Self) -> bool {
        self.ni == other.ni
    }

    pub(crate) fn to_map(&self) -> Value {
        unimplemented!()
    }

    pub(crate) fn from_map(_input: &Value) -> Option<Self> {
        unimplemented!()
    }

}

impl AsRef<NodeInfo> for KBucketEntry {
    fn as_ref(&self) -> &NodeInfo {
        &self.ni
    }
}

impl Into<NodeInfo> for KBucketEntry {
    fn into(self) -> NodeInfo {
        self.ni
    }
}

impl PartialEq for KBucketEntry {
    fn eq(&self, other: &Self) -> bool {
        self.ni == other.ni
    }

    fn ne(&self, other: &Self) -> bool {
        self.ni != other.ni
    }
}

impl Reachability for KBucketEntry {
    fn is_reachable(&self) -> bool {
        self.reachable
    }

    fn is_unreachable(&self) -> bool {
        self.last_sent == SystemTime::UNIX_EPOCH
    }

    fn set_reachable(&mut self, reachable: bool) {
        self.reachable = reachable
    }
}

impl fmt::Display for KBucketEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "{}@{};seen:{}; age:{}",
            self.ni.id(),
            self.ni.socket_addr(),
            as_secs!(self.last_seen),
            as_secs!(self.created)
        )?;

        if self.last_sent.elapsed().is_ok() {
            write!(f, "; sent:{}", as_secs!(self.last_sent))?;
        }
        if self.failed_requests > 0 {
            write!(f, "; fail: {}", self.failed_requests - 0)?;
        }
        if self.reachable {
            write!(f, "; reachable")?;
        }
        if self.ni.version() != 0 {
            write!(f,
                "; ver: {}",
                version::format_version(self.ni.version())
            )?;
        }
        Ok(())
    }
}
