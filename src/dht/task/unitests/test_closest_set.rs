use std::{
    rc::Rc,
    cell::RefCell,
};
use crate::{
    Id,
    NodeInfo,
    dht::task::{
        candidate_node::CandidateNode,
        closest_set::ClosestSet,
    }
};

fn make_candidate(distance: usize) -> Rc<RefCell<CandidateNode>> {
    let id = Id::try_from_bit_at(Id::BITS - distance).unwrap();
    let node = NodeInfo::new(
        id,
        format!("1.1.1.{}:39001", distance).parse().unwrap(),
    );
    Rc::new(RefCell::new(node.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reached_capacity() {
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
            .map(|entry| entry.borrow().id().clone())
            .collect::<Vec<_>>();
        let expected = [1, 2, 3, 4]
            .into_iter()
            .map(|distance| make_candidate(distance).borrow().id().clone())
            .collect::<Vec<_>>();

        assert!(closest.reached_capacity());

        assert_eq!(closest.size(), 4);
        assert_eq!(ids, expected);
        assert_eq!(closest.head(), expected[0]);
        assert_eq!(closest.tail(), expected[3]);
    }

    #[test]
    fn test_closest() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);
        let candidate = make_candidate(2);
        let id = candidate.borrow().id().clone();

        assert!(closest.is_empty());
        assert!(closest.entry(&id).is_none());
        assert!(!closest.contains(&id));

        closest.add(candidate.clone());

        assert!(closest.contains(&id));
        assert!(closest.entry(&id).is_some());

        let removed = closest.remove(&id);

        assert!(closest.is_empty());
        assert!(removed.is_some());
        assert!(closest.entry(&id).is_none());
        assert!(!closest.contains(&id));

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
    fn test_eligible() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);

        for distance in 1..=4 {
            closest.add(make_candidate(distance));
        }
        assert!(!closest.is_eligible());

        for distance in 5..=9 {
            closest.add(make_candidate(distance));
        }

        assert!(closest.insert_attempts_since_tail_modification() == 5);
        assert!(closest.is_eligible());
    }

    #[test]
    fn test_head_stable() {
        let target = Id::MIN_ID;
        let mut closest = ClosestSet::new(target, 4);

        for distance in (1..=4).rev() {
            closest.add(make_candidate(distance));
        }
        assert!(!closest.is_head_stable());

        for distance in 5..=9 {
            closest.add(make_candidate(distance));
        }

        assert!(closest.insert_attempts_since_head_modification() == 5);
        assert!(closest.is_head_stable());
    }
}
