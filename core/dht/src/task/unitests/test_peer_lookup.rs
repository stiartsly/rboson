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

use super::test_utils::make_test_dht;
use crate::{
    dht::DHT,
    task::{LookupTask, PeerLookupTask},
};
use boson_api::{Id, Network};
use std::rc::Rc;

fn make_dht() -> Rc<DHT> {
    make_test_dht(Network::IPv4, "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let target = Id::random();
        let dht = make_dht();
        let task = PeerLookupTask::new(dht.clone(), target.clone(), 7, 3, true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);
        assert!(task.result().is_empty());
    }
}
