use std::{
    fmt,
    cmp::{min, max},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, SystemTime}
};
use serde::{Serialize, Deserialize};
use crate::{
    Id,
    NodeInfo,
    core::version,
    dht::rpc::{Reachability, rpc_target::NodeInfoLike}
};

/**
 * Entry in a KBucket, it basically contains an IP address of a node,
 * the UDP port of the node and a node id.
 */
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerializableKbucketEntry", from = "SerializableKbucketEntry")]
pub(crate) struct KBucketEntry {
    ni          : NodeInfo,

    created     : SystemTime,
    last_seen   : SystemTime,
    last_sent   : SystemTime,

    reachable   : bool,
    failed_reqs : i32,
    avg_rtt     : Option<f64>,
}

impl KBucketEntry {
    const MAX_FAILURES: i32 = 5;
    const OLD_AND_STALE_FAILURES: i32 = 2;

    const OLD_AND_STALE_TIME: u64 = 15 * 60 * 1000; // 15 minutes
    const PING_BACKOFF_BASE_INTERVAL: u64 = 60 * 1000; // 1 minute
    const RTT_EMA_WEIGHT: f64 = 0.3;

    pub(crate) fn new(id: Id, addr: SocketAddr) -> Self {
        let now = SystemTime::now();
        Self {
            ni: NodeInfo::new(id, addr),
            created     : now,
            last_seen   : now,
            last_sent   : SystemTime::UNIX_EPOCH,
            reachable   : false,
            failed_reqs: 0,
            avg_rtt     : None,
        }
    }

    pub(crate) fn set_ver(&mut self, ver: i32) {
        self.ni.set_version(ver);
    }

    pub(crate) fn id(&self) -> &Id {
        &self.ni.id()
    }

    pub(crate) fn socket_addr(&self) -> &SocketAddr {
        self.ni.socket_addr()
    }

    pub(crate) fn created_time(&self) -> &SystemTime {
        &self.created
    }

    #[cfg(test)]
    pub(crate) fn last_seen(&self) -> &SystemTime {
        &self.last_seen
    }

    #[cfg(test)]
    pub(crate) fn set_last_seen(&mut self, last_seen: SystemTime) {
        self.last_seen = last_seen;
    }

    #[cfg(test)]
    pub(crate) fn last_sent(&self) -> &SystemTime {
        &self.last_sent
    }

    #[cfg(test)]
    pub(crate) const fn failed_reqs(&self) -> i32 {
        self.failed_reqs
    }

    pub(crate) const fn eligible_for_nodes_list(&self) -> bool {
        // 1 timeout can occasionally happen. should be fine to hand it out
        // as long as we've verified it at least once
        self.reachable && self.failed_reqs < 3
    }

    pub(crate) const fn eligible_for_local_lookup(&self) -> bool {
        // allow implicit initial ping during lookups
        // TO~DO: make this work now that we don't keep unverified entries
        // in the main bucket
        (self.reachable && self.failed_reqs <= 3) ||
            self.failed_reqs <= 0
    }

    fn backoff(&self) -> u64 {
        // Assertion in test case will guard the MAX_FAILURES not causing overflow
        Self::PING_BACKOFF_BASE_INTERVAL
            << min(Self::MAX_FAILURES, max(0, self.failed_reqs - 1))
    }

    #[allow(unused)]
    fn within_backoff_window_at(&self, _: &SystemTime) -> bool {
        self.failed_reqs != 0 && crate::elapsed_ms!(&self.last_sent) < self.backoff() as u128
    }

    #[allow(unused)]
    pub(crate) fn backoff_window_end(&self) -> Option<SystemTime> {
        if self.failed_reqs == 0 || self.last_sent == SystemTime::UNIX_EPOCH {
            return None;
        }

        Some(self.last_sent + Duration::from_millis(self.backoff() as u64))
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
        if crate::elapsed_ms!(self.last_seen) < 30 * 1000 ||
            self.within_backoff_window_at(&self.last_seen) {
            return false;
        }

        self.failed_reqs != 0
            || crate::elapsed_ms!(self.last_seen) > Self::OLD_AND_STALE_TIME as u128
    }

    pub(crate) fn old_and_stale(&self) -> bool {
        self.failed_reqs > Self::OLD_AND_STALE_FAILURES
            && crate::elapsed_ms!(self.last_seen) > Self::OLD_AND_STALE_TIME as u128
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
        self.failed_reqs > Self::MAX_FAILURES && !seen_since_last_sent
    }

    ///
	/// Determines if this entry needs to be replaced in the routing table.
	/// Replacement is needed:
    /// - if the node is unreachable with more than one failed request,
	/// - if it exceeds maximum allowed timeouts, or
    /// - if it is old and stale.
	///
	/// `true` if replacement is needed; `false` otherwise.
    ///
    pub(crate) fn needs_replacement(&self) -> bool {
        (self.failed_reqs > 1 && !self.is_reachable()) ||
            self.failed_reqs > Self::MAX_FAILURES ||
            self.old_and_stale()
    }

    pub(crate) fn merge(&mut self, entry: Self) {
        if !self.equals(&entry) {
            return;
        }

        if entry.last_seen > self.last_seen {
            self.failed_reqs = entry.failed_reqs;
        }
        if entry.is_reachable() {
            self.set_reachable(true);
        }

        if let Some(avg_rtt) = entry.avg_rtt {
            self.update_avg_rtt(avg_rtt);
        }

        self.created    = self.created.min(entry.created);
        self.last_seen  = self.last_seen.max(entry.last_seen);
        self.last_sent  = self.last_sent.max(entry.last_sent);
    }

    pub(crate) fn on_request_sent(&mut self) {
        self.last_sent = SystemTime::now();
    }

    pub(crate) fn update_last_sent(&mut self, last_sent: SystemTime) {
        self.last_sent = SystemTime::max(self.last_sent, last_sent);
    }

    pub(crate) fn on_responded(&mut self, rtt: u64) {
        self.last_seen = SystemTime::now();
        self.failed_reqs = 0;
        self.reachable = true;

        if rtt > 0 {
            self.update_avg_rtt(rtt as f64);
        }
    }

    pub(crate) fn on_timeout(&mut self) {
        self.failed_reqs += 1;
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.ni.matches(&other.ni)
    }

    pub(crate) fn equals(&self, other: &Self) -> bool {
        self.ni == other.ni
    }

    fn update_avg_rtt(&mut self, sample: f64) {
        self.avg_rtt = Some(match self.avg_rtt {
            Some(avg_rtt) => avg_rtt + Self::RTT_EMA_WEIGHT * (sample - avg_rtt),
            None => sample,
        });
    }
}

impl Eq for KBucketEntry {}
impl PartialEq for KBucketEntry {
    fn eq(&self, other: &Self) -> bool {
        self.ni.eq(&other.ni)
    }
}

impl Into<NodeInfo> for KBucketEntry {
    fn into(self) -> NodeInfo {
        self.ni
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

impl NodeInfoLike for KBucketEntry {
    fn ni(&self) -> NodeInfo {
        self.ni.clone()
    }

    fn id(&self) -> &Id {
        self.ni.id()
    }

    fn socket_addr(&self) -> &SocketAddr {
        self.ni.socket_addr()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerializableKbucketEntry {
    id: Id,
    addr: Vec<u8>,
    port: u16,
    #[serde(rename="created", skip_serializing_if = "crate::is_default")]
    created: u64,
    #[serde(rename="lastSeen", skip_serializing_if = "crate::is_default")]
    last_seen: u64,
    #[serde(rename="lastSent", skip_serializing_if = "crate::is_default")]
    last_sent: u64,
    #[serde(rename="reachable", skip_serializing_if = "crate::is_default")]
    reachable: bool,
   // #[serde(rename="failedRequests", skip_serializing_if = "crate::is_default")]
   // failed_reqs: i32,
    #[serde(rename="avgRtt", skip_serializing_if = "crate::is_default")]
    avg_rtt: Option<f64>,
    #[serde(rename="version", skip_serializing_if = "crate::is_default")]
    ver: i32,
}

impl Into<SerializableKbucketEntry> for KBucketEntry {
    fn into(self) -> SerializableKbucketEntry {
        let addr = match self.socket_addr().ip() {
            IpAddr::V4(addr4) => addr4.octets().to_vec(),
            IpAddr::V6(addr6) => addr6.octets().to_vec(),
        };

        SerializableKbucketEntry {
            id: self.id().clone(),
            addr,
            port        : self.socket_addr().port(),
            created     : crate::as_ms!(self.created) as u64,
            last_seen   : crate::as_ms!(self.last_seen) as u64,
            last_sent   : crate::as_ms!(self.last_sent) as u64,
            reachable   : self.reachable,
            // failed_reqs: self.failed_reqs,
            avg_rtt     : self.avg_rtt,
            ver         : self.ni.version(),
        }
    }
}

impl From<SerializableKbucketEntry> for KBucketEntry {
    fn from(ser: SerializableKbucketEntry) -> Self {
        let convert_cb = |ms | -> SystemTime {
            SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
        };

        let ip = match ser.addr.len() {
            4 => {
                let bytes: [u8; 4] = ser.addr.as_slice().try_into()
                    .expect("invalid IPv4 address length");
                IpAddr::V4(Ipv4Addr::from(bytes))
            },
            16 => {
                let bytes: [u8; 16] = ser.addr.as_slice().try_into()
                    .expect("invalid IPv6 address length");
                IpAddr::V6(Ipv6Addr::from(bytes))
            },
            _ => panic!("invalid IP address byte length"),
        };

        let mut entry = KBucketEntry::new(ser.id, SocketAddr::new(ip, ser.port));
        entry.set_ver(ser.ver);
        entry.created = convert_cb(ser.created);
        entry.last_seen = convert_cb(ser.last_seen);
        entry.last_sent = convert_cb(ser.last_sent);
        entry.reachable = ser.reachable;
        // entry.failed_reqs = ser.failed_reqs;
        entry.avg_rtt = ser.avg_rtt.filter(|rtt| rtt.is_finite() && *rtt >= 0.0);
        entry
    }
}

impl fmt::Display for KBucketEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "{}@{};seen:{}; age:{}",
            self.ni.id(),
            self.ni.socket_addr(),
            crate::as_secs!(self.last_seen),
            crate::as_secs!(self.created)
        )?;

        if self.last_sent.elapsed().is_ok() {
            write!(f, "; sent:{}", crate::as_secs!(self.last_sent))?;
        }
        if self.failed_reqs > 0 {
            write!(f, "; fail: {}", self.failed_reqs - 0)?;
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
