use std::{
    fmt,
    cmp::Ordering
};
use crate::core::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prefix {
    depth: i32,
    id: Id,
}

impl Prefix {
    pub(crate) fn new() -> Self {
        Self {
            id: Id::default(),
            depth: -1
        }
    }

    #[cfg(test)]
    pub(crate) fn from(src: &Id, depth: i32) -> Self {
       assert!(depth < Id::BITS as i32);

        let mut id = Id::default();
        Id::bits_copy(src, &mut id, depth);

        Self {id, depth }
    }

    pub(crate) const fn id(&self) -> &Id {
        &self.id
    }

    #[allow(unused)]
    pub(crate) const fn depth(&self) -> i32 {
        self.depth
    }

    pub(crate) fn is_prefix_of(&self, id: &Id) -> bool {
        Id::bits_equal(&self.id, id, self.depth)
    }

    pub(crate) const fn is_splittable(&self) -> bool {
        self.depth < (Id::BITS - 1) as i32
    }

    pub(crate) fn first(&self) -> Id {
        self.id.clone()
    }

    pub(crate) fn last(&self) -> Id {
        let prefix = Prefix {
            id: Id::MAX_ID,
            depth: self.depth,
        };
        let trailing_bits = prefix.id.distance(&Id::MAX_ID);
        self.id.distance(&trailing_bits)
    }

    #[allow(unused)]
    pub(crate) fn parent(&self) -> Prefix {
        let mut parent = self.clone();
        if self.depth == -1 {
            return parent;
        }

        // set last bit to zero
        parent.set_tail(parent.depth);
        parent.depth -= 1;
        parent
    }

    pub(crate) fn split_branch(&self, high_branch: bool) -> Prefix {
        let mut branch = self.clone();
        branch.depth += 1;

        let depth = branch.depth as usize;
        branch.id.update(|bytes| {
            let val = 0x80 >> (depth % 8);
            if high_branch {
                bytes[depth / 8] |= val;
            } else {
                bytes[depth / 8] &= !val;
            }
        });
        branch
    }

    #[allow(unused)]
    pub(crate) fn is_sibling_of(&self, other: &Prefix) -> bool {
        self.depth == other.depth &&
            Id::bits_equal(&self.id, &other.id, self.depth - 1)
    }

    pub(crate) fn random_id(&self) -> Id {
        let mut id = Id::random();
        Id::bits_copy(&self.id, &mut id, self.depth);
        id
    }

    fn set_tail(&mut self, bit: i32) {
        let index = bit >> 3;
        self.id.update(|bytes| {
            bytes[index as usize] &= !(0x80 >> (bit & 0x07))
        });
    }
}

impl Ord for Prefix {
    fn cmp(&self, other: &Prefix) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Prefix {
    fn partial_cmp(&self, other: &Prefix) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.depth == -1 {
            write!(f, "all")?;
        }

        let id_slice = hex::encode({
            let end_index = (self.depth + 8) >> 3;
            self.id.as_bytes()[..end_index as usize].to_vec()
        });

        write!(f, "{}/{}", id_slice, self.depth)?;
        Ok(())
    }
}
