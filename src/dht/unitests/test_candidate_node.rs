use crate::{Id, NodeInfo};
use crate::dht::{
    node_entry::Reachability,
    routing::kbucket_entry::KBucketEntry,
    task::candidate_node::CandidateNode,
};

fn make_node() -> NodeInfo {
     NodeInfo::new(
        Id::random(),
        "1.1.1.1:39001".parse().unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_candidate() {
        let node = make_node();
        let cn = CandidateNode::from(node.clone());

        assert_eq!(cn.id(), node.id());
        assert_eq!(cn.pinged(), 0);
        assert_eq!(cn.token(), 0);
        assert_eq!(cn.is_sent(), false);
        assert_eq!(cn.is_replied(), false);
        assert_eq!(cn.is_acked(), false);
        assert_eq!(cn.is_inflight(), false);
        assert_eq!(cn.is_eligible(), true);
        assert_eq!(cn.is_reachable(), false);
        assert_eq!(cn.is_unreachable(), false);
        assert_eq!(cn.as_ref().socket_addr(), node.socket_addr());
    }

    #[test]
    fn test_set_sent_and_clear_sent() {
        let node = make_node();
        let mut cn = CandidateNode::from(node);

        cn.set_sent();

        assert_eq!(cn.is_sent(), true);
        assert_eq!(cn.is_inflight(), true);
        assert_eq!(cn.pinged(), 1);
        assert_eq!(cn.is_eligible(), false);

        cn.clear_sent();

        assert_eq!(cn.is_sent(), false);
        assert_eq!(cn.is_inflight(), false);
        assert_eq!(cn.pinged(), 1);
        assert_eq!(cn.is_eligible(), true);
    }

    #[test]
    fn test_replied_and_acked() {
        let node = make_node();
        let mut cn = CandidateNode::from(node);

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
        let mut cn = CandidateNode::from(node);

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

    #[test]
    fn test_candiate_from_kbucket_entry() {
        let mut entry = KBucketEntry::new(
            Id::random(),
            "1.1.1.1:39001".parse().unwrap(),
        );
        entry.set_reachable(true);

        let cn = CandidateNode::from(entry.clone());

        assert_eq!(cn.id(), entry.id());
        assert_eq!(cn.is_reachable(), true);
        assert_eq!(cn.as_ref().socket_addr(), entry.socket_addr());
    }
}
