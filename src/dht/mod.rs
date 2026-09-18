mod msg {
    pub(crate) mod announce_peer_req;
    pub(crate) mod error;
    pub(crate) mod find_node_req;
    pub(crate) mod find_node_rsp;
    pub(crate) mod find_peer_req;
    pub(crate) mod find_peer_rsp;
    pub(crate) mod find_value_req;
    pub(crate) mod find_value_rsp;
    pub(crate) mod lookup_req;
    pub(crate) mod lookup_rsp;
    pub(crate) mod msg;
    pub(crate) mod store_value_req;

    #[cfg(test)]
    mod unitests {
        mod test_announce_peer_req;
        mod test_error;
        mod test_find_node_req;
        mod test_find_node_rsp;
        mod test_find_peer_req;
        mod test_find_peer_rsp;
        mod test_find_value_req;
        mod test_find_value_rsp;
        mod test_msg;
        mod test_store_value_req;
    }

    pub(crate) use {
        announce_peer_req::AnnouncePeerRequest,
        error::Error as ErrorBody,
        find_node_req::FindNodeRequest,
        find_node_rsp::FindNodeResponse,
        find_peer_req::FindPeerRequest,
        find_peer_rsp::FindPeerResponse,
        find_value_req::FindValueRequest,
        find_value_rsp::FindValueResponse,
        lookup_req::LookupRequest,
        lookup_rsp::LookupResponse,
        msg::{Body, Message},
        store_value_req::StoreValueRequest,
    };
}

mod task {
    pub(crate) mod candidate_node;
    pub(crate) mod closest_candidates;
    pub(crate) mod closest_set;

    pub(crate) mod lookup_task;
    pub(crate) mod task;
    pub(crate) mod task_listener;
    pub(crate) mod task_manager;

    pub(crate) mod node_lookup;
    pub(crate) mod peer_announce;
    pub(crate) mod peer_lookup;
    pub(crate) mod ping_refresh;
    pub(crate) mod value_announce;
    pub(crate) mod value_lookup;

    #[cfg(test)]
    mod unitests {
        mod test_candidate_node;
        mod test_closest_candidates;
        mod test_closest_set;
        mod test_node_lookup;
        mod test_peer_announce;
        mod test_peer_lookup;
        mod test_task_manager;
        mod test_utils;
        mod test_value_announce;
        mod test_value_lookup;
    }

    pub(crate) use {
        candidate_node::CandidateNode,
        closest_candidates::ClosestCandidates,
        closest_set::ClosestSet,
        lookup_task::{LookupTask, LookupTaskData},
        node_lookup::NodeLookupTask,
        peer_announce::PeerAnnounceTask,
        peer_lookup::PeerLookupTask,
        ping_refresh::PingRefreshTask,
        task::{Task, TaskData},
        value_announce::ValueAnnounceTask,
        value_lookup::ValueLookupTask,
    };
}

mod routing {
    pub(crate) mod kbucket;
    pub(crate) mod kbucket_entry;
    pub(crate) mod kclosest_nodes;
    pub(crate) mod prefix;
    pub(crate) mod routing_table;

    #[cfg(test)]
    mod unitests {
        mod test_kbucket_entry;
        mod test_kclosest_nodes;
        mod test_prefix;
        mod test_routing_table;
    }

    pub(crate) use {
        kbucket::KBucket, kbucket_entry::KBucketEntry, kclosest_nodes::KClosestNodes,
        prefix::Prefix, routing_table::RoutingTable,
    };
}

mod rpc {
    pub(crate) mod listener;
    pub(crate) mod rpc_server;
    pub(crate) mod rpc_target;
    pub(crate) mod rpccall;

    pub(crate) use {
        listener::Listener,
        rpc_target::{Target, TargetInfo},
        rpccall::RpcCall,
    };
}

mod cached_identity;
mod dht;
mod dht_verticle;
mod eligible_peers;
mod eligible_value;
mod node_verticle;
mod storage;
mod suspicious_node_detector;
mod token_manager;

pub mod connection_status;
pub mod connection_status_listener;
pub mod errors;
pub mod lookup_option;
pub mod node;
pub mod node_options;

pub use crate::dht::{
    connection_status::ConnectionStatus,
    connection_status_listener::ConnectionStatusListener,
    lookup_option::LookupOption,
    node::Node,
    node_options::{NodeOptions, DEFAULT_DHT_PORT},
};

pub(crate) mod utils {
    use std::net::{IpAddr, SocketAddr};

    pub(crate) fn is_broadcast(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => v4.is_broadcast(),
            IpAddr::V6(_) => false,
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
            IpAddr::V4(v4) => v4.is_private(),
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
        !(ip.is_loopback()
            || ip.is_multicast()
            || ip.is_unspecified()
            || is_broadcast(ip)
            || is_linklocal(ip)
            || is_sitelocal(ip)
            || is_mapped_ipv4(ip))
    }

    pub(crate) fn is_any_unicast(ip: &IpAddr) -> bool {
        is_global_unicast(ip) || is_sitelocal(ip)
    }

    pub(crate) fn is_bogon(addr: &SocketAddr) -> bool {
        !(addr.port() > 0 && addr.port() < 0xFFFF && is_global_unicast(&addr.ip()))
    }

    #[allow(unused)]
    pub(crate) fn local_addr(ipv4: bool) -> Option<IpAddr> {
        let if_addrs = match get_if_addrs::get_if_addrs() {
            Ok(v) => v,
            Err(_) => return None,
        };

        for iface in if_addrs {
            let ip = iface.ip();
            if !ip.is_loopback() && ((ipv4 && ip.is_ipv4()) || (!ipv4 && ip.is_ipv6())) {
                return Some(ip);
            }
        }
        None
    }
}

#[cfg(test)]
mod unitests {
    mod test_addr;
    mod test_cached_identity;
    mod test_dht;
    mod test_node;
    mod test_rpccall;
    mod test_token_manager;

    // storage
    mod test_storage;
}
