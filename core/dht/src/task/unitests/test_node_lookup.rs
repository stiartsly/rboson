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
    rpc::rpc_target::TargetInfo,
    task::{LookupTask, NodeLookupTask},
};
use boson_api::{Id, Network, NodeInfo};
use std::rc::Rc;

fn make_dht() -> Rc<DHT> {
    make_test_dht(Network::IPv4, "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_defaults() {
        let target = Id::random();
        let dht = make_dht();
        let task = NodeLookupTask::new(dht.clone(), target.clone(), true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);

        assert!(!task.is_bootstrap());
        assert!(!task.want_token());
        assert!(!task.want_target());
    }

    #[test]
    fn test_task_with_options() {
        let target = Id::random();
        let dht = make_dht();
        let mut task = NodeLookupTask::new(dht.clone(), target.clone(), false);

        task.with_bootstrap(true);
        task.with_want_token(true);
        task.with_want_target(true);

        assert_eq!(task.target(), &target);
        assert_eq!(task.candidate_size(), 0);

        assert!(task.is_bootstrap());
        assert!(task.want_token());
        assert!(task.want_target());
    }

    #[test]
    fn test_inject_candidates() {
        let target = Id::random();
        let dht = make_dht();
        let mut task = NodeLookupTask::new(dht.clone(), target, false);

        let candidate = NodeInfo::new(Id::random(), "1.1.1.1:39011".parse().unwrap());
        task.with_inject_candidates(vec![candidate.clone()]);
        assert_eq!(task.candidate_size(), 1);

        let next = task.next_candidate().expect("candidate should be present");
        let next = next.borrow();
        assert_eq!(next.id(), candidate.id());
        assert_eq!(next.addr(), candidate.addr());
    }
}
