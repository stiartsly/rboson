pub(crate) const DEFAULT_DHT_PORT: u16 = 39001;

pub(crate) const MAX_ENTRIES_PER_BUCKET: usize = 8;

// Refresh interval for a bucket in milliseconds
pub(crate) const BUCKET_REFRESH_INTERVAL: u128 = 15 * 60 * 1000;

// Maximum number of timeouts for considering a K-bucket entry as old and stale
pub(crate) const KBUCKET_OLD_AND_STALE_TIMEOUT: i32 = 2;

pub(crate) const ROUTING_TABLE_MAINTENANCE_INTERVAL: u128 = 4 * 60 * 1000;

// Time threshold for considering a K-bucket entry as old and stale in milliseconds
pub(crate) const KBUCKET_OLD_AND_STALE_TIME: u128 = 15 * 60 * 1000;

// Base interval for backoff when sending ping messages to nodes in milliseconds
pub(crate) const KBUCKET_PING_BACKOFF_BASE_INTERVAL: u128 = 60 * 1000;

// Maximum number of timeouts before considering a K-bucket entry as unresponsive
pub(crate) const KBUCKET_MAX_TIMEOUTS: i32 = 5;

pub(crate) const RE_ANNOUNCE_INTERVAL: u64 = 5 * 60 * 1000;


pub(crate) const DHT_UPDATE_INTERVAL:u64 = 10000;
pub(crate) const RANDOM_LOOKUP_INTERVAL: u64 = 10 * 60 * 1000;  // 10 minutes
pub(crate) const RANDOM_PING_INTERVAL: u64 = 10 * 1000;         // 10 seconds

pub(crate) const RPC_SERVER_REACHABILITY_TIMEOUT: u128 = 60 * 1000;

pub(crate) const BOOTSTRAP_IF_LESS_THAN_X_PEERS:usize = 30;
pub(crate) const SELF_LOOKUP_INTERVAL:u128 = 30 * 60 * 1000;   // 30 minutes
pub(crate) const ROUTING_TABLE_PERSIST_INTERVAL: u64 = 10 * 60 * 1000;   // 10 minutes
// pub(crate) const BOOTSTRAP_MIN_INTERVAL: u128 = 4 * 60 * 1000;


pub(crate) const RPC_CALL_TIMEOUT_MAX: u64 = 10 * 1000;

pub(crate) const EXPIRED_CHECK_INTERVAL: u64 = 60 * 1000;

pub(crate) const STORAGE_EXPIRE_INTERVAL: u64 = 5 * 60 * 1000;

pub(crate) const MAX_PEER_AGE: u128 = 120 * 60 * 1000;
pub(crate) const MAX_VALUE_AGE: u128 = 120 * 60 * 1000;
