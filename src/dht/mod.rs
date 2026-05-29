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

    pub(crate) use {
        msg::{Message, Body},
        lookup_req::LookupRequest,
        lookup_rsp::LookupResponse,
    };
}

mod task {
    pub(crate) mod closest_candidates;
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

    pub(crate) use {
        task::{Task, TaskData},
        lookup_task::{LookupTask, LookupTaskData},
        closest_candidates::ClosestCandidates,
        closest_set::ClosestSet,
        candidate_node::CandidateNode,
        peer_lookup::PeerLookupTask,
        node_lookup::NodeLookupTask,
        peer_announce::PeerAnnounceTask,
        value_lookup::ValueLookupTask,
        value_announce::ValueAnnounceTask,
    };
}

mod routing {
    pub(crate) mod prefix;
    pub(crate) mod kbucket;
    pub(crate) mod kbucket_entry;
    pub(crate) mod kclosest_nodes;
    pub(crate) mod routing_table;

    pub(crate) use {
        prefix::Prefix,
        kbucket::KBucket,
        kbucket_entry::KBucketEntry,
        kclosest_nodes::KClosestNodes,
        routing_table::RoutingTable,
    };
}

mod rpc {
    pub(crate) mod listener;
    pub(crate) mod rpccall;
    pub(crate) mod rpc_server;
    pub(crate) mod rpc_target;

    pub(crate) use {
        rpccall::RpcCall,
        rpc_target::{Reachability, Target},
        listener::Listener,
    };
}

mod cached_identity;
mod dht;
mod timer;
mod token_manager;

mod consumer;
mod promise;
mod eligible_peers;
mod eligible_value;
mod suspicious_node_detector;

pub mod connection_status_listener;
pub mod connection_status;
pub mod node_status;
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
    cfg::yaml_configuration::NodeConfiguration,
};

pub(crate) mod utils {
    use std::net::{
        SocketAddr,
        IpAddr
    };

    pub(crate) fn is_broadcast(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => v4.is_broadcast(),
            IpAddr::V6(_) => false
        }
    }

    pub(crate) fn is_linklocal(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => v4.is_link_local(),
            IpAddr::V6(v6) => {
                let v = &v6.octets();
                v[0] == 0xfe && v[1] == 0x80
            }
        }
    }

    pub(crate) fn is_sitelocal(ip: &IpAddr) -> bool {
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

    pub(crate) fn is_mapped_ipv4(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(_) => return false,
            IpAddr::V6(v6) => {
                let mapped_ipv4_prefix = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
                let octets = v6.octets().to_vec();
                octets == mapped_ipv4_prefix
            }
        }
    }

    pub(crate) fn is_global_unicast(ip: &IpAddr) -> bool {
        !(ip.is_loopback() ||
            ip.is_multicast() ||
            ip.is_unspecified() ||
            is_broadcast(ip) ||
            is_linklocal(ip) ||
            is_sitelocal(ip) ||
            is_mapped_ipv4(ip))
    }

    pub(crate) fn is_any_unicast(ip: &IpAddr) -> bool {
        is_global_unicast(ip) || is_sitelocal(ip)
    }

    pub(crate) fn is_bogon(addr: &SocketAddr) -> bool {
        !(addr.port() > 0 &&
        addr.port() < 0xFFFF && is_global_unicast(&addr.ip()))
    }
}

#[cfg(test)]
mod unitests {
    mod test_addr;
    mod test_node_configuration;

    // routingtable
    mod test_prefix;
    mod test_kclosest_nodes;
    mod test_routing_table;
    mod test_kbucket_entry;

    mod test_rpccall;
    mod test_token_manager;
    mod test_dht;
    mod test_cached_identity;

    // msg
    mod test_find_node_req;
    mod test_find_node_rsp;
    mod test_announce_peer_req;
    mod test_find_peer_req;
    mod test_find_peer_rsp;
    mod test_find_value_req;
    mod test_find_value_rsp;
    mod test_store_value_req;
    mod test_error;

    // task
    mod test_candidate_node;
    mod test_closest_candidates;
    mod test_closest_set;
    mod test_node_lookup;
    mod test_peer_lookup;
    mod test_value_lookup;
    mod test_peer_announce;
    mod test_value_announce;

    // storage
    mod test_storage;
}
