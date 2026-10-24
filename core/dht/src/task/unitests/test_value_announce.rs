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
    task::{
        candidate_node::CandidateNode,
        closest_set::ClosestSet,
        task::{State, Task},
        value_announce::ValueAnnounceTask,
    },
};
use boson_api::{Id, Network, NodeInfo, Value};
use std::{cell::RefCell, rc::Rc};

fn make_dht() -> Rc<DHT> {
    make_test_dht(Network::IPv4, "127.0.0.1")
}

fn make_value() -> Value {
    Value::packed(Some(Id::random()), None, None, None, vec![1, 2, 3, 4], 5)
}

fn make_closestset(token: i32) -> ClosestSet {
    let target = Id::random();
    let candidate = NodeInfo::new(Id::random(), "1.1.1.1:39011".parse().unwrap());
    let mut cn: CandidateNode = candidate.into();
    cn.set_token(token);

    let mut closest = ClosestSet::new(target, 8);
    closest.add(Rc::new(RefCell::new(cn)));
    closest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let value = make_value();
        let dht = make_dht();

        let task = ValueAnnounceTask::new(dht, value, 7);
        assert!(task.is_unstarted());
        assert!(task.is_done());
    }

    #[test]
    fn test_task_with_closestset() {
        let value = make_value();
        let dht = make_dht();

        let task = ValueAnnounceTask::new(dht, value, -1);
        assert!(task.is_done());

        task.with_closest(make_closestset(42));
        assert!(!task.is_done());
    }

    #[test]
    fn test_cancel() {
        let value = make_value();
        let dht = make_dht();
        let mut task = ValueAnnounceTask::new(dht, value, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        task.set_state_if(&State::Initialized, State::Running);
        task.iterate();
        assert!(task.is_done());
        assert!(task.is_running());

        task.cancel();
        assert!(task.is_done());
        assert!(task.is_canceled());
    }

    #[test]
    fn test_complete() {
        let value = make_value();
        let dht = make_dht();
        let mut task = ValueAnnounceTask::new(dht, value, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        task.set_state_if(&State::Initialized, State::Running);
        task.iterate();
        assert!(task.is_done());
        assert!(task.is_running());

        task.complete();
        assert!(task.is_done());
        assert!(task.is_completed());
    }

    #[test]
    fn test_start() {
        let value = make_value();
        let dht = make_dht();
        let mut task = ValueAnnounceTask::new(dht, value, -1);
        assert!(task.is_unstarted());
        assert!(task.is_done());

        task.with_closest(make_closestset(0));
        assert!(task.is_unstarted());
        assert!(!task.is_done());

        task.start();
        assert!(task.is_done());
        assert!(task.is_completed());
    }
}
