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

use std::{any::Any, cell::RefCell, collections::VecDeque, rc::Rc};

use crate::{
    dht::DHT,
    msg::msg,
    task::{CandidateNode, ClosestSet, Task, TaskData},
};
use boson_api::{EasyHandler, Value};

const MAX_TODO_ENTRIES: usize = 24;

pub(crate) struct ValueAnnounceTask {
    base_data: TaskData,

    todo: Rc<RefCell<VecDeque<Rc<RefCell<CandidateNode>>>>>,
    value: Value,
    expected_seq: i32,

    dht: Rc<DHT>,
}

impl ValueAnnounceTask {
    pub(crate) fn new(dht: Rc<DHT>, value: Value, expected_seq: i32) -> Self {
        Self {
            base_data: TaskData::new(),
            todo: Rc::new(RefCell::new(VecDeque::with_capacity(MAX_TODO_ENTRIES))),
            value,
            expected_seq,
            dht,
        }
    }

    pub(crate) fn with_closest(&self, closest: ClosestSet) -> &Self {
        let mut borrowed_todo = self.todo.borrow_mut();
        let mut entries = closest.entries();

        while let Some(cn) = entries.pop() {
            if borrowed_todo.len() >= MAX_TODO_ENTRIES {
                break;
            }
            borrowed_todo.push_back(cn);
        }
        log::debug!(
            "{}#{} added {} nodes to announce queue",
            self.task_name(),
            self.task_id(),
            borrowed_todo.len()
        );
        self
    }
}

impl Task for ValueAnnounceTask {
    fn data(&self) -> &TaskData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut TaskData {
        &mut self.base_data
    }

    fn as_task(&self) -> &dyn Task {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dht(&self) -> Rc<DHT> {
        self.dht.clone()
    }

    fn iterate(&mut self) {
        while self.can_dorequest() {
            let cn = match self.todo.borrow().front() {
                Some(cn) => cn.clone(),
                _ => break,
            };

            let token = cn.borrow().token();
            if token == 0 {
                self.todo.borrow_mut().pop_front();
                continue;
            }
            let msg = msg::store_value_request(self.value.clone(), token, self.expected_seq);

            let cloned_todo = self.todo.clone();
            let handler = EasyHandler::new(move |_| {
                cloned_todo.borrow_mut().pop_front();
            });

            self.send_call(cn.into(), msg, Some(handler));
        }
    }

    fn is_done(&self) -> bool {
        self.todo.borrow().is_empty() && self.data().is_done()
    }
}
