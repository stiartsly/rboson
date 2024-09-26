use std::fmt;
use std::cmp::Ordering;

use crate::{
    id, Id,
    id::ID_BITS,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Prefix {
    depth: i32,
    id: Id,
}

impl Prefix {
    pub fn new() -> Self {
        Self {
            id: Id::default(),
            depth: -1
        }
    }

    pub fn from_id(src: &Id, depth: i32) -> Self {
       assert!(depth < ID_BITS as i32);

        let mut id = Id::default();
        id::bits_copy(src, &mut id, depth);

        Self {id, depth }
    }

    pub const fn id(&self) -> &Id {
        &self.id
    }

    pub const fn depth(&self) -> i32 {
        self.depth
    }

    pub fn is_prefix_of(&self, id: &Id) -> bool {
        id::bits_equal(&self.id, id, self.depth)
    }

    pub const fn is_splittable(&self) -> bool {
        self.depth < (id::ID_BITS - 1) as i32
    }

    pub fn first(&self) -> Id {
        self.id.clone()
    }

    pub fn last(&self) -> Id {
        let prefix = Prefix {
            id: id::MAX_ID,
            depth: self.depth,
        };
        let trailing_bits = prefix.id.distance(&id::MAX_ID);
        self.id.distance(&trailing_bits)
    }

    pub fn parent(&self) -> Prefix {
        let mut parent = self.clone();
        if self.depth == -1 {
            return parent;
        }

        // set last bit to zero
        parent.set_tail(parent.depth);
        parent.depth -= 1;
        parent
    }

    pub fn split_branch(&self, high_branch: bool) -> Prefix {
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

    pub fn is_sibling_of(&self, other: &Prefix) -> bool {
        self.depth == other.depth &&
            id::bits_equal(&self.id, &other.id, self.depth - 1)
    }

    pub fn random_id(&self) -> Id {
        let mut id = Id::random();
        id::bits_copy(&self.id, &mut id, self.depth);
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
