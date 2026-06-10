use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use log::{debug, info, trace};

use crate::core::Id;

const SUSPICIOUS_OBSERVATION_HITS: usize = 8;
const SUSPICIOUS_HITS_THRESHOLD: usize = 32;
const DEFAULT_OBSERVATION_PERIOD: Duration = Duration::from_secs(15 * 60);
const DEFAULT_BAN_DURATION: Duration = Duration::from_secs(30 * 60);

#[allow(dead_code)]
pub trait SuspiciousNodeDetector : Send + Sync {
    fn is_suspicious_with_expected(&self, addr: &SocketAddr, expected: Option<&Id>) -> bool;

    fn is_suspicious(&self, addr: &SocketAddr) -> bool {
        self.is_suspicious_with_expected(addr, None)
    }

    fn is_banned(&self, host: &IpAddr) -> bool;

    fn is_banned_addr(&self, addr: &SocketAddr) -> bool {
        self.is_banned(&addr.ip())
    }

    fn last_known_id(&self, addr: &SocketAddr) -> Option<&Id>;
    fn observe(&mut self, addr: SocketAddr, id: Id);
    fn malformed_message(&mut self, addr: SocketAddr);
    fn inconsistent(&mut self, addr: SocketAddr, id: Option<Id>);
    fn observed_size(&self) -> usize;
    fn banned_size(&self) -> usize;
    fn purge(&mut self);
    fn clear(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuspiciousActivity {
    None,
    Inconsistent,
    MalformedMessage,
}

#[derive(Debug, Clone)]
struct ObservationRecord {
    last_id: Option<Id>,
    last_activity: SuspiciousActivity,
    hits: usize,
    expires_at: Instant,
}

impl ObservationRecord {
    fn new(id: Option<Id>, activity: SuspiciousActivity, expires_at: Instant) -> Self {
        Self {
            last_id: id,
            last_activity: activity,
            hits: usize::from(activity != SuspiciousActivity::None),
            expires_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DefaultSuspiciousNodeDetector {
    observation_period: Duration,
    observation_hit_threshold: usize,
    ban_duration: Duration,
    observed_nodes: HashMap<SocketAddr, ObservationRecord>,
    observed_hosts: HashMap<IpAddr, ObservationRecord>,
    banned_nodes: HashMap<IpAddr, Instant>,
}

impl Default for DefaultSuspiciousNodeDetector {
    fn default() -> Self {
        Self::new(
            DEFAULT_OBSERVATION_PERIOD,
            SUSPICIOUS_HITS_THRESHOLD,
            DEFAULT_BAN_DURATION,
        )
    }
}

unsafe impl Send for ObservationRecord {}
unsafe impl Sync for ObservationRecord {}

impl DefaultSuspiciousNodeDetector {
    pub fn new(
        observation_period: Duration,
        observation_hit_threshold: usize,
        ban_duration: Duration,
    ) -> Self {
        assert!(
            !observation_period.is_zero(),
            "observation period must be positive"
        );
        assert!(
            observation_hit_threshold > 0,
            "observation hit threshold must be positive"
        );
        assert!(!ban_duration.is_zero(), "ban duration must be positive");

        Self {
            observation_period,
            observation_hit_threshold,
            ban_duration,
            observed_nodes: HashMap::new(),
            observed_hosts: HashMap::new(),
            banned_nodes: HashMap::new(),
        }
    }

    fn observe_activity(
        &mut self,
        addr: SocketAddr,
        id: Option<Id>,
        activity: SuspiciousActivity,
    ) {
        if self.is_banned(&addr.ip()) {
            return;
        }

        let now = Instant::now();
        let expires_at = now + self.observation_period;

        let mut should_ban_host = false;
        match self.observed_nodes.entry(addr) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                trace!("New observation for {}: id={:?}, activity={:?}", addr, id, activity);
                entry.insert(ObservationRecord::new(id, activity, expires_at));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let record = entry.get_mut();
                if activity != SuspiciousActivity::None || record.last_id != id {
                    record.hits += 1;
                }

                if record.hits >= self.observation_hit_threshold {
                    info!(
                        "Node at {} marked suspicious: activity={:?}, hits={}",
                        addr.ip(),
                        activity,
                        record.hits
                    );
                    should_ban_host = true;
                } else {
                    record.last_activity = activity;
                    record.last_id = id;
                    record.expires_at = expires_at;
                    trace!(
                        "Updated observation for address {}: id={:?}, state={:?}, hits={}",
                        addr,
                        record.last_id,
                        record.last_activity,
                        record.hits
                    );
                }
            }
        }

        if should_ban_host {
            self.observed_nodes.remove(&addr);
            self.ban_node(addr.ip(), now + self.ban_duration);
        }

        if activity == SuspiciousActivity::None {
            return;
        }

        let host = addr.ip();
        let mut should_ban_host_from_host_record = false;
        match self.observed_hosts.entry(host) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                trace!("New observation for host {}: activity={:?}", host, activity);
                entry.insert(ObservationRecord::new(None, activity, expires_at));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let record = entry.get_mut();
                record.hits += 1;
                if record.hits >= self.observation_hit_threshold {
                    info!(
                        "Host {} marked suspicious: activity={:?}, hits={}",
                        host,
                        activity,
                        record.hits
                    );
                    should_ban_host_from_host_record = true;
                } else {
                    record.last_activity = activity;
                    record.expires_at = expires_at;
                    trace!(
                        "Updated observation for host {}: state={:?}, hits={}",
                        host,
                        record.last_activity,
                        record.hits
                    );
                }
            }
        }

        if should_ban_host_from_host_record {
            self.observed_hosts.remove(&host);
            self.ban_node(host, now + self.ban_duration);
        }

        if let Some(id) = id {
            let related_addrs = self
                .observed_nodes
                .iter()
                .filter_map(|(observed_addr, record)| {
                    (record.last_id.as_ref() == Some(&id)).then_some(*observed_addr)
                })
                .collect::<Vec<_>>();

            if related_addrs.len() >= SUSPICIOUS_OBSERVATION_HITS {
                for related_addr in related_addrs {
                    info!(
                        "Id {} marked suspicious, ban related host {}",
                        id,
                        related_addr.ip()
                    );
                    self.observed_nodes.remove(&related_addr);
                    self.observed_hosts.remove(&related_addr.ip());
                    self.ban_node(related_addr.ip(), now + self.ban_duration);
                }
            }
        }
    }

    fn ban_node(&mut self, host: IpAddr, expiration_time: Instant) {
        match self.banned_nodes.insert(host, expiration_time) {
            None => info!("Promote the marked node {} to suspicious node", host),
            Some(_) => debug!("Extended suspicious for host {}", host),
        }
    }
}

impl SuspiciousNodeDetector for DefaultSuspiciousNodeDetector {
    fn is_suspicious_with_expected(&self, addr: &SocketAddr, expected: Option<&Id>) -> bool {
        if self.banned_nodes.contains_key(&addr.ip()) {
            return true;
        }

        let Some(record) = self.observed_nodes.get(addr) else {
            return false;
        };

        match expected {
            Some(expected) => record.last_id.as_ref() != Some(expected),
            None => record.hits >= SUSPICIOUS_OBSERVATION_HITS,
        }
    }

    fn is_banned(&self, host: &IpAddr) -> bool {
        self.banned_nodes.contains_key(host)
    }

    fn last_known_id(&self, addr: &SocketAddr) -> Option<&Id> {
        self.observed_nodes
            .get(addr)
            .and_then(|record| record.last_id.as_ref())
    }

    fn observe(&mut self, addr: SocketAddr, id: Id) {
        self.observe_activity(addr, Some(id), SuspiciousActivity::None);
    }

    fn malformed_message(&mut self, addr: SocketAddr) {
        self.observe_activity(addr, None, SuspiciousActivity::MalformedMessage);
    }

    fn inconsistent(&mut self, addr: SocketAddr, id: Option<Id>) {
        self.observe_activity(addr, id, SuspiciousActivity::Inconsistent);
    }

    fn observed_size(&self) -> usize {
        self.observed_nodes.len() + self.observed_hosts.len()
    }

    fn banned_size(&self) -> usize {
        self.banned_nodes.len()
    }

    fn purge(&mut self) {
        let now = Instant::now();
        self.observed_nodes.retain(|addr, record| {
            let keep = now <= record.expires_at;
            if !keep {
                debug!("Removed expired observation for address {}", addr);
            }
            keep
        });

        self.observed_hosts.retain(|host, record| {
            let keep = now <= record.expires_at;
            if !keep {
                debug!("Removed expired observation for host {}", host);
            }
            keep
        });

        self.banned_nodes.retain(|host, expires_at| {
            let keep = now <= *expires_at;
            if !keep {
                debug!("Removed expired suspicious node {}", host);
            }
            keep
        });
    }

    fn clear(&mut self) {
        self.observed_nodes.clear();
        self.observed_hosts.clear();
        self.banned_nodes.clear();
    }
}

impl fmt::Display for DefaultSuspiciousNodeDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.observed_nodes.is_empty() && self.observed_hosts.is_empty() && self.banned_nodes.is_empty() {
            return f.write_str("Empty");
        }

        if !self.observed_nodes.is_empty() || !self.observed_hosts.is_empty() {
            writeln!(f, "Observed[{}]:", self.observed_size())?;
            for (addr, record) in &self.observed_nodes {
                writeln!(
                    f,
                    "  {}, {:?}, {}, {:?}",
                    addr,
                    record.last_activity,
                    record.hits,
                    record.expires_at.saturating_duration_since(Instant::now())
                )?;
            }
            for (host, record) in &self.observed_hosts {
                writeln!(
                    f,
                    "  {}, {:?}, {}, {:?}",
                    host,
                    record.last_activity,
                    record.hits,
                    record.expires_at.saturating_duration_since(Instant::now())
                )?;
            }
        }

        if !self.banned_nodes.is_empty() {
            writeln!(f, "Banned[{}]:", self.banned_nodes.len())?;
            for (host, expires_at) in &self.banned_nodes {
                writeln!(
                    f,
                    "  {}, {:?}",
                    host,
                    expires_at.saturating_duration_since(Instant::now())
                )?;
            }
        }

        Ok(())
    }
}

unsafe impl Send for DefaultSuspiciousNodeDetector {}
unsafe impl Sync for DefaultSuspiciousNodeDetector {}
