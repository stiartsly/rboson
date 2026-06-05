use std::sync::{Arc, Mutex};
use crate::{
    Id,
    NodeInfo,
    dht::task::{
        candidate_node::CandidateNode,
        closest_set::ClosestSet,
    }
};

fn make_candidate(distance: usize) -> Arc<Mutex<CandidateNode>> {
    let id = Id::try_from_bit_at(Id::BITS - distance).unwrap();
    let node = NodeInfo::new(
        id,
        format!("1.1.1.{}:39001", distance).parse().unwrap(),
    );
    Arc::new(Mutex::new(CandidateNode::from(node)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);

        closest.add(make_candidate(4));
        closest.add(make_candidate(2));
        closest.add(make_candidate(1));
        closest.add(make_candidate(3));
        closest.add(make_candidate(6));

        let ids = closest
            .entries()
            .into_iter()
            .map(|entry| entry.lock().unwrap().id().clone())
            .collect::<Vec<_>>();
        let expected = [1, 2, 3, 4]
            .into_iter()
            .map(|distance| make_candidate(distance).lock().unwrap().id().clone())
            .collect::<Vec<_>>();

        assert_eq!(closest.size(), 4);
        assert_eq!(closest.reached_capacity(), true);
        assert_eq!(ids, expected);
        assert_eq!(closest.head(), expected[0]);
        assert_eq!(closest.tail(), expected[3]);
    }

    #[test]
    fn test_contains() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);
        let candidate = make_candidate(2);
        let id = candidate.lock().unwrap().id().clone();

        assert_eq!(closest.is_empty(), true);
        assert_eq!(closest.contains(&id), false);
        assert_eq!(closest.entry(&id).is_none(), true);

        closest.add(candidate.clone());

        assert_eq!(closest.contains(&id), true);
        assert_eq!(closest.entry(&id).is_some(), true);

        let removed = closest.remove(&id);
        assert_eq!(removed.is_some(), true);
        assert_eq!(closest.contains(&id), false);
        assert_eq!(closest.entry(&id).is_none(), true);
        assert_eq!(closest.is_empty(), true);
    }

    #[test]
    fn test_empty_closest() {
        let target = Id::MIN_ID;
        let closest = ClosestSet::new(target, 4);
        let fallback = target.distance(&Id::MAX_ID);

        assert_eq!(closest.head(), fallback);
        assert_eq!(closest.tail(), fallback);
    }

    #[test]
    fn test_is_eligible() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);

        for distance in 1..=4 {
            closest.add(make_candidate(distance));
        }
        assert_eq!(closest.is_eligible(), false);

        for distance in 5..=9 {
            closest.add(make_candidate(distance));
        }

        assert_eq!(closest.insert_attempts_since_tail_modification(), 5);
        assert_eq!(closest.is_eligible(), true);
    }

    #[test]
    fn test_is_head_stable() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);

        for distance in (1..=4).rev() {
            closest.add(make_candidate(distance));
        }
        assert_eq!(closest.is_head_stable(), false);

        for distance in 5..=9 {
            closest.add(make_candidate(distance));
        }

        assert_eq!(closest.insert_attempts_since_head_modification(), 5);
        assert_eq!(closest.is_head_stable(), true);
    }
}
