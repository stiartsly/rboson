pub(crate) mod storage;
pub mod errors;
mod cfg {
    pub(crate) mod node_config;
    pub(crate) mod yaml_configuration;
}

mod msg {
    pub(crate) mod msg;
    pub(crate) mod error;

    pub(crate) mod lookup_req;
    pub(crate) mod lookup_rsp;
    pub(crate) mod find_node_req;
    pub(crate) mod find_node_rsp;
    pub(crate) mod find_peer_req;
    pub(crate) mod find_peer_rsp;
    pub(crate) mod find_value_req;
    pub(crate) mod find_value_rsp;

    pub(crate) mod announce_peer_req;
    pub(crate) mod store_value_req;
}

mod task {
    mod closest_candidates;
    pub(crate) mod closest_set;
    pub(crate) mod candidate_node;

    pub(crate) mod task_manager;

    pub(crate) mod task;
    pub(crate) mod task_listener;
    pub(crate) mod lookup_task;

    pub(crate) mod ping_refresh;
    pub(crate) mod node_lookup;
    pub(crate) mod peer_lookup;
    pub(crate) mod peer_announce;
    pub(crate) mod value_lookup;
    pub(crate) mod value_announce;
}

mod routing {
    pub(crate) mod prefix;
    pub(crate) mod kbucket;
    pub(crate) mod kbucket_entry;
    pub(crate) mod kclosest_nodes;
    pub(crate) mod routing_table;
}

mod cached_identity;
mod dht;
mod server;
mod rpccall;
mod scheduler;
mod scheduler_error;
mod token_manager;
mod node_entry;
mod node_status;

mod promise;
mod eligible_peers;
mod eligible_value;
mod suspicious_node_detector;

pub mod connection_status_listener;
pub mod connection_status;
pub mod node_status_listener;
pub mod lookup_option;
pub mod node;

pub use crate::dht::{
    lookup_option::LookupOption,
    connection_status::ConnectionStatus,
    connection_status_listener::ConnectionStatusListener,
    node_status_listener::NodeStatusListener,
    node::Node,
    cfg::node_config::NodeConfig,
    cfg::yaml_configuration::YamlNodeConfiguration,
};

#[cfg(test)]
mod unitests {
    mod test_prefix;
    mod test_yaml_configuration;
    mod test_kclosest_nodes;
    mod test_routing_table;
    mod test_scheduler;
    mod test_kbucket_entry;
    mod test_rpccall;
    mod test_token_manager;

    /*
    mod test_addr;
   // mod test_sqlite_storage;
    mod test_token_man;
    mod test_find_node_req;
    mod test_find_node_rsp;
    mod test_find_peer_req;
    mod test_find_peer_rsp;
    mod test_find_value_req;
    mod test_find_value_rsp;
   // mod test_node_runner;
    */

    use std::{fs, env};

    fn working_path(input: &str) -> String {
        let path = env::current_dir().unwrap().join(input);
        if !fs::metadata(&path).is_ok() {
            match fs::create_dir(&path) {
                Ok(_) => {}
                Err(e) => {
                    panic!("Failed to create directory: {}", e);
                }
            }
        }
        path.display().to_string()
    }

    fn remove_working_path(input: &str) {
        if fs::metadata(&input).is_ok() {
            match fs::remove_dir_all(&input) {
                Ok(_) => {}
                Err(e) => {
                    panic!("Failed to remove directory: {}", e);
                }
            }
        }
    }
}

#[macro_export]
macro_rules! addr_family {
    ($val:expr) => {{
        match $val.is_ipv4() {
            true => "ipv4",
            false => "ipv6"
        }
    }};
}

use std::net::{
    SocketAddr,
    IpAddr
};

fn is_broadcast(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_broadcast(),
        IpAddr::V6(_) => false
    }
}

fn is_linklocal(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => {
            let v = &v6.octets();
            v[0] == 0xfe && v[1] == 0x80
        }
    }
}

fn is_sitelocal(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
        },
        IpAddr::V6(v6) => {
            let v = &v6.octets();
            v[0] == 0xfc || v[0] == 0xfd
        }
    }
}

fn is_mapped_ipv4(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(_) => return false,
        IpAddr::V6(v6) => {
            let mapped_ipv4_prefix = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
            let octets = v6.octets().to_vec();
            octets == mapped_ipv4_prefix
        }
    }
}

fn is_global_unicast(ip: &IpAddr) -> bool {
    !(ip.is_loopback() ||
        ip.is_multicast() ||
        ip.is_unspecified() ||
        is_broadcast(ip) ||
        is_linklocal(ip) ||
        is_sitelocal(ip) ||
        is_mapped_ipv4(ip))
}

fn is_any_unicast(ip: &IpAddr) -> bool {
    is_global_unicast(ip) || is_sitelocal(ip)
}

fn is_bogon(addr: &SocketAddr) -> bool {
   !(addr.port() > 0 &&
     addr.port() < 0xFFFF && is_global_unicast(&addr.ip()))
}
