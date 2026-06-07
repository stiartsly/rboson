use crate::{Id, NodeInfo};
use crate::dht::{
    rpc::{rpc_target::NodeInfoLike, Reachability},
    routing::KBucketEntry,
    task::CandidateNode,
};

fn make_node() -> NodeInfo {
     NodeInfo::new(
        Id::random(),
        "1.1.1.1:39001".parse().unwrap(),
    )
}

fn make_bucket_entry() -> KBucketEntry {
    let mut entry = KBucketEntry::new(
        Id::random(),
        "1.1.1.1:39001".parse().unwrap(),
    );
    entry.set_reachable(true);
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_node() {
        let node = make_node();
        let mut cn: CandidateNode = node.clone().into();

        assert_eq!(cn.id(), node.id());
        assert_eq!(cn.pinged(), 0);
        assert_eq!(cn.token(), 0);
        assert_eq!(cn.socket_addr(), node.socket_addr());

        assert!(!cn.is_sent());
        assert!(!cn.is_replied());
        assert!(!cn.is_acked());
        assert!(!cn.is_inflight());
        assert!(!cn.is_reachable());
        assert!(!cn.is_unreachable());
        assert!(cn.is_eligible());

        cn.set_sent();
        assert!(cn.is_sent());
        assert!(cn.is_inflight());
        assert!(cn.pinged() == 1);
        assert!(!cn.is_unreachable());

        cn.set_replied();
        assert!(!cn.is_sent());
        assert!(cn.is_replied());

        cn.set_acked();
        assert!(cn.is_acked());

        cn.clear_sent();
        assert!(!cn.is_sent());
        assert!(!cn.is_inflight());
        assert!(cn.is_eligible());

        cn.set_reachable(true);
        assert!(cn.is_reachable());

        for _ in 0..5 {
            cn.set_sent();
        }
        assert!(cn.is_unreachable());
        assert!(!cn.is_eligible());
    }

    #[test]
    fn test_from_bucket_entry() {
        let entry = make_bucket_entry();
        let cn: CandidateNode = entry.clone().into();

        assert_eq!(cn.id(), entry.id());
        assert_eq!(cn.pinged(), 0);
        assert_eq!(cn.token(), 0);
        assert_eq!(cn.socket_addr(), entry.socket_addr());

        assert!(!cn.is_sent());
        assert!(!cn.is_replied());
        assert!(!cn.is_acked());
        assert!(!cn.is_inflight());
        assert!(cn.is_eligible());
        assert!(cn.is_reachable());
        assert!(!cn.is_unreachable());
    }

    #[test]
    fn test_replied_and_acked() {
        let node = make_node();
        let mut cn: CandidateNode = node.clone().into();

        assert_eq!(cn.is_replied(), false);
        assert_eq!(cn.is_acked(), false);
        assert_eq!(cn.token(), 0);

        cn.set_replied();
        cn.set_acked();
        cn.set_token(77);

        assert_eq!(cn.is_replied(), true);
        assert_eq!(cn.is_acked(), true);
        assert_eq!(cn.token(), 77);
    }

    #[test]
    fn test_reachability() {
        let node = make_node();
        let mut cn: CandidateNode = node.clone().into();

        assert_eq!(cn.is_reachable(), false);
        assert_eq!(cn.is_unreachable(), false);

        cn.set_reachable(true);
        assert_eq!(cn.is_reachable(), true);
        assert_eq!(cn.is_unreachable(), false);

        cn.set_sent();
        cn.clear_sent();
        cn.set_sent();
        cn.clear_sent();
        cn.set_sent();

        assert_eq!(cn.pinged(), 3);
        assert_eq!(cn.is_unreachable(), true);
        assert_eq!(cn.is_eligible(), false);
    }
}
