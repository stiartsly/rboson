use std::sync::{Arc, Mutex};
use std::net::SocketAddr;
use std::time::SystemTime;

use crate::{
    PeerInfo,
    NodeInfo,
    cryptobox,
    signature,
};

#[macro_export]
macro_rules! srv_endp {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().remote_name).as_str()
    };
}

#[macro_export]
macro_rules! srv_addr {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().remote_addr)
    };
}

#[macro_export]
macro_rules! srv_nodeid {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().remote_node).lock().unwrap().id()
    };
}

#[macro_export]
macro_rules! srv_peer {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().remote_peer)
    };
}

#[macro_export]
macro_rules! ups_endp {
    ($managed:expr) => {
       unwrap!($managed.lock().unwrap().upstream_name).as_str()
    };
}

#[macro_export]
macro_rules! ups_addr {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().upstream_addr)
    };
}

#[macro_export]
macro_rules! ups_peer {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().peer)
    };
}

#[macro_export]
macro_rules! session_keypair {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().session_keypair)
    };
}

#[macro_export]
macro_rules! enbox {
    ($managed:expr) => {
        unwrap!($managed.lock().unwrap().cryptobox)
    };
}

pub(crate) struct ManagedFields {
    pub(crate) session_keypair:     Option<cryptobox::KeyPair>,
    pub(crate) cryptobox:           Option<cryptobox::CryptoBox>,

    pub(crate) remote_peer:         Option<Arc<Mutex<PeerInfo>>>,
    pub(crate) remote_node:         Option<Arc<Mutex<NodeInfo>>>,
    pub(crate) remote_addr:         Option<SocketAddr>,
    pub(crate) remote_name:         Option<String>,

    pub(crate) upstream_addr:       Option<SocketAddr>,
    pub(crate) upstream_name:       Option<String>,

    pub(crate) domain_enabled:      bool,
    pub(crate) relay_port:          Option<u16>,
    pub(crate) peer_keypair:        Option<signature::KeyPair>,
    pub(crate) peer_domain:         Option<String>,
    pub(crate) peer:                Option<PeerInfo>,

    pub(crate) server_failures:     i32,
    pub(crate) reconnect_delay:     u128,

    pub(crate) inflights:           usize,
    pub(crate) connections:         usize,
    pub(crate) capacity:            usize,

    pub(crate) last_idle_check:     SystemTime,
    pub(crate) last_announce_peer:  SystemTime,
    pub(crate) last_save_peer:      SystemTime,

    //pub(crate) last_health_check:   SystemTime,
    //pub(crate) last_reconnect:      SystemTime
}

impl ManagedFields {
    pub(crate) fn new() -> Self {
        Self {
            session_keypair:    Some(cryptobox::KeyPair::random()),
            cryptobox:          None,

            remote_node:        None,
            remote_peer:        None,
            remote_addr:        None,
            remote_name:        None,

            upstream_addr:      None,
            upstream_name:      None,

            domain_enabled:     false,
            peer_keypair:       None,
            peer_domain:        None,
            peer:               None,
            relay_port:         None,

            server_failures:    0,
            reconnect_delay:    0,

            inflights:          0,
            connections:        0,
            capacity:           25,

            last_idle_check:    SystemTime::UNIX_EPOCH,
            last_announce_peer: SystemTime::UNIX_EPOCH,
            last_save_peer:     SystemTime::UNIX_EPOCH
        }
    }

    fn reset(&mut self) {
        self.session_keypair    = Some(cryptobox::KeyPair::random());
        self.cryptobox          = None;

        self.server_failures    = 0;
        self.reconnect_delay    = 0;

        self.inflights          = 0;
        self.connections        = 0;

        self.last_idle_check    = SystemTime::UNIX_EPOCH;
        self.last_announce_peer = SystemTime::UNIX_EPOCH;
        self.last_save_peer     = SystemTime::UNIX_EPOCH;
    }

    pub(crate) fn is_authenticated(&self) -> bool {
        self.cryptobox.is_some()
    }

    pub(crate) fn needs_new_connection(&mut self) -> bool {
        if self.connections >= self.capacity {
            return false;
        }

        /*
        if self.last_reconnect.elapsed()).as_millis() < self.reconnect_delay {
            return false;
        }
        */

        if self.connections == 0 {
            if self.is_authenticated() {
                self.reset();
            }
            return true;
        }

        if self.inflights == self.connections {
            return true;
        }

        false   // TODO: refine the conditions later.
    }
}
