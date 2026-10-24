// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::{routing::prefix::Prefix};
use boson_api::{Id};

/*
* APIs for testcases.
- Prefix::new()
- Prefix::from_id(id, depth);
- id()
- depth()
- is_prefix_of()
- is_splittable()
- first()
- last()
- parent()
- split_branch(high_branch)
- is_sibling_of(..)
- random_id()
- Eq
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let min = Id::min();
        let prefix = Prefix::new();
        assert_eq!(prefix.id(), &min);
        assert_eq!(prefix.is_prefix_of(&min), true);
        assert_eq!(prefix.depth(), -1);
    }

    #[test]
    fn test_from_id() {
        let id = Id::random();
        let prefix = Prefix::from(&id, 5);
        assert_eq!(prefix.is_prefix_of(&id), true);
        assert_eq!(prefix.depth(), 5);
    }

    #[test]
    fn test_is_prefix_of() {
        let hexstr = "0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let rc = Id::try_from(hexstr);
        assert_eq!(rc.is_ok(), true);
        let id = rc.unwrap();
        assert_eq!(id.to_hexstr(), hexstr);

        let prefix = Prefix::from(&id, 64);
        assert_eq!(prefix.is_prefix_of(&id), true);

        let hexstr = "0x4833af415161cbd0ffffffffffffffffffffffffffffffffffffffffffffffff";
        let rc = Id::try_from(hexstr);
        assert_eq!(rc.is_ok(), true);
        let id = rc.unwrap();
        assert_eq!(id.to_hexstr(), hexstr);
        assert_eq!(prefix.is_prefix_of(&id), true);

        let hexstr = "0x4833af415161cbd1f3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let rc = Id::try_from(hexstr);
        assert_eq!(rc.is_ok(), true);
        let id = rc.unwrap();
        assert_eq!(id.to_hexstr(), hexstr);
        assert_eq!(prefix.is_prefix_of(&id), false);
    }

    #[test]
    fn test_is_splitable() {
        for i in 0..(Id::BITS as i32 - 2) {
            let id = Id::random();
            let prefix = Prefix::from(&id, i);
            assert_eq!(prefix.is_splittable(), true);
        }

        let id = Id::random();
        let prefix = Prefix::from(&id, Id::BITS as i32 - 1);
        assert_eq!(prefix.is_splittable(), false);
    }

    #[test]
    fn test_is_sibling_of() {
        let hexstr = "0x4833af415161cbd0a3ef83aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let rc = Id::try_from(hexstr);
        assert_eq!(rc.is_ok(), true);
        let id = rc.unwrap();
        let prefix = Prefix::from(&id, 84);

        let hexstr = "0x4833af415161cbd0a3ef8faa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let rc = Id::try_from(hexstr);
        assert_eq!(rc.is_ok(), true);
        let id = rc.unwrap();
        let prefix2 = Prefix::from(&id, 84);

        let hexstr = "0x4833af415161cbd0a3ef93aa59a55fbadc9bd520a886a8fa214a3d09b6676cb8";
        let rc = Id::try_from(hexstr);
        assert_eq!(rc.is_ok(), true);
        //let id = rc.unwrap();
        //let prefix3 = Prefix::from(&id, 84);

        assert_eq!(prefix2.is_sibling_of(&prefix), true);
        // TODO: assert_eq!(prefix3.is_sibling_of(&prefix), false);
    }

    #[test]
    fn test_first() {
        for i in 0..Id::BITS as i32 - 1 {
            let id = Id::random();
            let prefix = Prefix::from(&id, i);
            let first = prefix.first();
            assert_eq!(prefix.is_prefix_of(&first), true);
        }
    }

    #[test]
    fn test_last() {
        for i in 0..Id::BITS as i32 - 1 {
            let id = Id::random();
            let prefix = Prefix::from(&id, i);
            let last = prefix.last();
            assert_eq!(prefix.is_prefix_of(&last), true);
        }
    }

    #[test]
    fn test_parent() {
        let id = Id::random();
        let prefix = Prefix::from(&id, usize::MAX as i32);
        let parent = prefix.parent();
        assert_eq!(prefix == parent, true);
    }

    #[test]
    fn test_randomid() {
        for i in 0..Id::BITS as i32 {
            let id = Id::random();
            let prefix = Prefix::from(&id, i);
            let rand_id = prefix.random_id();

            assert_eq!(prefix.is_prefix_of(&id), true);
            assert_eq!(prefix.is_prefix_of(&rand_id), true);
            // assert_eq!(Id::bits_equal(id, rand_id, i), true);
        }
    }

    #[test]
    fn test_split_branch() {
        for i in 0..Id::BITS as i32 - 1 {
            let id = Id::random();
            let prefix = Prefix::from(&id, i);
            let pl = prefix.split_branch(false);
            let ph = prefix.split_branch(true);

            assert_eq!(prefix.is_prefix_of(pl.id()), true);
            assert_eq!(prefix.is_prefix_of(ph.id()), true);
            assert_eq!(prefix == pl.parent(), true);
            assert_eq!(prefix == ph.parent(), true);
        }
    }

    #[test]
    fn test_eq() {
        let id = Id::random();
        let prefix1 = Prefix::from(&id, 5);
        let prefix2 = Prefix::from(&id, 5);
        assert_eq!(prefix1, prefix2);
    }
}
