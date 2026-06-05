use crate::{Id, NodeInfo};
use crate::dht::task::closest_candidates::ClosestCandidates;

fn make_node(distance: usize, host: &str, port: u16) -> NodeInfo {
    NodeInfo::new(
        Id::try_from_bit_at(Id::BITS - distance).unwrap(),
        format!("{host}:{port}").parse().unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let target = Id::MIN_ID;
        let mut candidates = ClosestCandidates::new(target, 3);

        let node4 = make_node(4, "1.1.1.4", 39004);
        let node2 = make_node(2, "1.1.1.2", 39002);
        let node1 = make_node(1, "1.1.1.1", 39001);
        let node3 = make_node(3, "1.1.1.3", 39003);

        candidates.add(vec![
            node4.clone(),
            node2.clone(),
            node1.clone(),
            node3.clone()
        ]);

        let expected = vec![
            node1.id().clone(),
            node2.id().clone(),
            node3.id().clone()
        ];

        assert_eq!(candidates.size(), 3);
        assert_eq!(candidates.reached_capacity(), true);
        assert_eq!(candidates.is_empty(), false);
        //assert_eq!(candidates.ids(), expected);
        assert_eq!(candidates.head(), node1.id().clone());
        assert_eq!(candidates.tail(), node3.id().clone());
        assert_eq!(candidates.candidate_node(node1.id()).is_some(), true);
        assert_eq!(candidates.candidate_node(node4.id()).is_none(), true);
    }

    #[test]
    fn test_add_dedups() {
        let target = Id::MIN_ID;
        let mut candidates = ClosestCandidates::new(target, 4);

        let first = make_node(3, "1.1.1.1", 39001);
        let second = make_node(1, "1.1.1.1", 39002);

        candidates.add(vec![
            first.clone(),
            second.clone()
        ]);

        assert_eq!(candidates.size(), 1);
        assert_eq!(candidates.candidate_node(first.id()).is_some(), true);
        assert_eq!(candidates.candidate_node(second.id()).is_none(), true);
    }

    #[test]
    fn test_add_when_developer_mode_enabled() {
        let target = Id::MIN_ID;
        let mut candidates = ClosestCandidates::with_developer_mode(target, 4, true);

        let first = make_node(3, "1.1.1.1", 39001);
        let second = make_node(1, "1.1.1.1", 39002);

        candidates.add(vec![first.clone(), second.clone()]);

        assert_eq!(candidates.size(), 2);
        assert_eq!(candidates.candidate_node(first.id()).is_some(), true);
        assert_eq!(candidates.candidate_node(second.id()).is_some(), true);
    }

    #[test]
    fn test_remove() {
        let target = Id::MIN_ID;
        let mut candidates = ClosestCandidates::new(target, 4);

        let keep = make_node(1, "1.1.1.1", 39001);
        let remove_a = make_node(2, "1.1.1.2", 39002);
        let remove_b = make_node(3, "1.1.1.3", 39003);

        candidates.add(vec![keep.clone(), remove_a.clone(), remove_b.clone()]);
        candidates.remove_if(|cn| {
            cn.lock().unwrap().as_ref().socket_addr().port() != 39001
        });

        assert_eq!(candidates.size(), 1);
        assert_eq!(candidates.candidate_node(keep.id()).is_some(), true);
        assert_eq!(candidates.candidate_node(remove_a.id()).is_none(), true);
        assert_eq!(candidates.candidate_node(remove_b.id()).is_none(), true);

        candidates.add(vec![remove_a.clone(), remove_b.clone()]);
        assert_eq!(candidates.size(), 1);
    }

    #[test]
    fn test_remove_and_readd() {
        let target = Id::MIN_ID;
        let mut candidates = ClosestCandidates::new(target, 2);

        let first = make_node(1, "1.1.1.1", 39001);
        let second = make_node(2, "1.1.1.2", 39002);
        let third = make_node(3, "1.1.1.3", 39003);

        candidates.add(vec![first.clone(), second.clone(), third.clone()]);
        assert_eq!(candidates.candidate_node(third.id()).is_none(), true);

        let removed = candidates.remove(first.id());
        assert_eq!(removed.is_some(), true);

        candidates.add(vec![first.clone()]);
        assert_eq!(candidates.candidate_node(first.id()).is_none(), true);

        candidates.add(vec![third.clone()]);
        assert_eq!(candidates.candidate_node(third.id()).is_some(), true);
    }

    #[test]
    fn test_next() {
        let target = Id::MIN_ID;
        let mut candidates = ClosestCandidates::new(target, 4);

        let closest = make_node(1, "1.1.1.1", 39001);
        let middle = make_node(2, "1.1.1.2", 39002);
        let farthest = make_node(3, "1.1.1.3", 39003);
        candidates.add(vec![
            farthest.clone(),
            closest.clone(),
            middle.clone()
        ]);

        let closest_candidate = candidates.candidate_node(closest.id()).unwrap();
        closest_candidate.lock().unwrap().set_sent();

        let middle_candidate = candidates.candidate_node(middle.id()).unwrap();
        middle_candidate.lock().unwrap().set_sent();
        middle_candidate.lock().unwrap().clear_sent();

        let next = candidates.next().unwrap();
        assert_eq!(next.lock().unwrap().id(), middle.id());
    }

    #[test]
    fn test_empty() {
        let target = Id::MIN_ID;
        let candidates = ClosestCandidates::new(target, 4);
        let fallback = target.distance(&Id::MAX_ID);

        assert_eq!(candidates.head(), fallback);
        assert_eq!(candidates.tail(), fallback);
        assert_eq!(candidates.next().is_none(), true);
    }
}