use std::fmt;
use std::cmp::{min, max};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::result::Result as SResult;
use std::time::{Duration, SystemTime};

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
};

use crate::{
    as_secs,
    elapsed_ms,
    Id,
    NodeInfo,
    core::version,
};
use crate::dht::{
    node_entry::Reachability,
    server::RpcServer,
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
    avg_rtt: Option<f64>,
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
            failed_requests: 0,
            avg_rtt: None,
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

    pub(crate) fn is_never_contacted(&self) -> bool {
        self.last_sent == SystemTime::UNIX_EPOCH
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

    fn within_backoff_window_at(&self, _: &SystemTime) -> bool {
        self.failed_requests != 0 && elapsed_ms!(&self.last_sent) < self.backoff() as u128
    }

    pub(crate) fn within_backoff_window(&self) -> bool {
        self.within_backoff_window_at(&SystemTime::now())
    }

    pub(crate) fn backoff_window_end(&self) -> Option<SystemTime> {
        if self.failed_requests == 0 || self.last_sent == SystemTime::UNIX_EPOCH {
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
        if elapsed_ms!(self.last_seen) < 30 * 1000 ||
            self.within_backoff_window_at(&self.last_seen) {
            return false;
        }

        self.failed_requests != 0
            || elapsed_ms!(self.last_seen) > Self::OLD_AND_STALE_TIME as u128
    }

    pub(crate) fn old_and_stale(&self) -> bool {
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
        if std::ptr::eq(self, entry) || !self.equals(entry) {
            return;
        }

        if entry.last_seen > self.last_seen {
            self.failed_requests = entry.failed_requests;
        }
        if entry.is_reachable() {
            self.set_reachable(true);
        }

        if let Some(avg_rtt) = entry.avg_rtt {
            self.update_avg_rtt(avg_rtt);
        }

        self.created = self.created.min(entry.created);
        self.last_seen = self.last_seen.max(entry.last_seen);
        self.last_sent = self.last_sent.max(entry.last_sent);
    }

    pub(crate) fn rtt(&self) -> u64 {
        self.rtt_with(RpcServer::RPC_CALL_TIMEOUT_MAX)
    }

    pub(crate) fn rtt_with(&self, default_rtt: u64) -> u64 {
        match self.avg_rtt {
            Some(avg_rtt) if avg_rtt.is_finite() => avg_rtt.clamp(0.0, default_rtt as f64).round() as u64,
            _ => default_rtt,
        }
    }

    pub(crate) fn on_request_sent(&mut self) {
        self.last_sent = SystemTime::now();
    }

    pub(crate) fn update_last_sent(&mut self, last_sent: SystemTime) {
        self.last_sent = SystemTime::max(self.last_sent, last_sent);
    }

    pub(crate) fn on_responded(&mut self, rtt: u64) {
        self.last_seen = SystemTime::now();
        self.failed_requests = 0;
        self.reachable = true;

        if rtt > 0 {
            self.update_avg_rtt(rtt as f64);
        }
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

    fn update_avg_rtt(&mut self, sample: f64) {
        self.avg_rtt = Some(match self.avg_rtt {
            Some(avg_rtt) => avg_rtt + Self::RTT_EMA_WEIGHT * (sample - avg_rtt),
            None => sample,
        });
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

impl Serialize for KBucketEntry {
    fn serialize<S>(&self, ser: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut len = 7; // id, addr, port, created, last_seen, last_sent, failed_requests
        if self.ni.version() != 0 { len += 1; }
        if self.reachable { len += 1; }
        if self.avg_rtt.is_some() { len += 1; }

        let mut state = ser.serialize_struct("KBucketEntry", len)?;
        let addr = match self.socket_addr().ip() {
            IpAddr::V4(addr4) => addr4.octets().to_vec(),
            IpAddr::V6(addr6) => addr6.octets().to_vec(),
        };

        state.serialize_field("id", self.id())?;
        state.serialize_field("addr", &addr)?;
        state.serialize_field("port", &self.socket_addr().port())?;
        state.serialize_field("c", &Self::system_time_to_millis(self.created))?;
        state.serialize_field("ls", &Self::system_time_to_millis(self.last_seen))?;
        state.serialize_field("lt", &Self::system_time_to_millis(self.last_sent))?;
        state.serialize_field("f", &self.failed_requests)?;

        if self.ni.version() != 0 {
            state.serialize_field("ver", &self.ni.version())?;
        }
        if self.reachable {
            state.serialize_field("r", &self.reachable)?;
        }
        if let Some(avg_rtt) = self.avg_rtt {
            state.serialize_field("rt", &avg_rtt)?;
        }

        state.end()
    }
}

impl<'de> Deserialize<'de> for KBucketEntry {
    fn deserialize<D>(des: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Id,
            Addr,
            Port,
            Version,
            Created,
            LastSeen,
            LastSent,
            Reachable,
            FailedRequests,
            AvgRtt,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(des: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(des)?;
                match key.as_str() {
                    "id" => Ok(Field::Id),
                    "addr" => Ok(Field::Addr),
                    "port" => Ok(Field::Port),
                    "ver" => Ok(Field::Version),
                    "c" => Ok(Field::Created),
                    "ls" => Ok(Field::LastSeen),
                    "lt" => Ok(Field::LastSent),
                    "r" => Ok(Field::Reachable),
                    "f" => Ok(Field::FailedRequests),
                    "rt" => Ok(Field::AvgRtt),
                    _ => Err(de::Error::unknown_field(&key, &["id", "addr", "port", "ver", "c", "ls", "lt", "r", "f", "rt"])),
                }
            }
        }

        struct KBucketEntryVisitor;

        impl<'de> Visitor<'de> for KBucketEntryVisitor {
            type Value = KBucketEntry;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct KBucketEntry")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut id: Option<Id> = None;
                let mut addr: Option<Vec<u8>> = None;
                let mut port: Option<u16> = None;
                let mut version = 0;
                let mut created_ms: Option<u64> = None;
                let mut last_seen_ms: Option<u64> = None;
                let mut last_sent_ms: Option<u64> = None;
                let mut reachable = false;
                let mut failed_requests = 0;
                let mut avg_rtt: Option<f64> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Id => id = Some(map.next_value()?),
                        Field::Addr => addr = Some(map.next_value()?),
                        Field::Port => port = Some(map.next_value()?),
                        Field::Version => version = map.next_value()?,
                        Field::Created => created_ms = Some(map.next_value()?),
                        Field::LastSeen => last_seen_ms = Some(map.next_value()?),
                        Field::LastSent => last_sent_ms = Some(map.next_value()?),
                        Field::Reachable => reachable = map.next_value()?,
                        Field::FailedRequests => failed_requests = map.next_value()?,
                        Field::AvgRtt => avg_rtt = Some(map.next_value()?),
                    }
                }

                let id = id.ok_or_else(|| de::Error::missing_field("id"))?;
                let addr = addr.ok_or_else(|| de::Error::missing_field("addr"))?;
                let port = port.ok_or_else(|| de::Error::missing_field("port"))?;
                let created_ms = created_ms.ok_or_else(|| de::Error::missing_field("c"))?;
                let last_seen_ms = last_seen_ms.ok_or_else(|| de::Error::missing_field("ls"))?;
                let last_sent_ms = last_sent_ms.ok_or_else(|| de::Error::missing_field("lt"))?;

                let ip = match addr.len() {
                    4 => {
                        let bytes: [u8; 4] = addr.as_slice().try_into()
                            .map_err(|_| de::Error::custom("invalid IPv4 address length"))?;
                        IpAddr::V4(Ipv4Addr::from(bytes))
                    },
                    16 => {
                        let bytes: [u8; 16] = addr.as_slice().try_into()
                            .map_err(|_| de::Error::custom("invalid IPv6 address length"))?;
                        IpAddr::V6(Ipv6Addr::from(bytes))
                    },
                    _ => return Err(de::Error::custom("invalid IP address byte length")),
                };

                let mut entry = KBucketEntry::new(id, SocketAddr::new(ip, port));
                entry.set_ver(version);
                entry.created = KBucketEntry::millis_to_system_time(created_ms);
                entry.last_seen = KBucketEntry::millis_to_system_time(last_seen_ms);
                entry.last_sent = KBucketEntry::millis_to_system_time(last_sent_ms);
                entry.reachable = reachable;
                entry.failed_requests = failed_requests;
                entry.avg_rtt = avg_rtt.filter(|rtt| rtt.is_finite() && *rtt >= 0.0);
                Ok(entry)
            }
        }

        des.deserialize_map(KBucketEntryVisitor)
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

impl KBucketEntry {
    fn system_time_to_millis(time: SystemTime) -> u64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn millis_to_system_time(ms: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
    }
}
