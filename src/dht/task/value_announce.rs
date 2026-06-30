use std::{
    any::Any,
    rc::Rc,
    cell::RefCell,
    collections::VecDeque,
};

use crate::Value;
use crate::dht::{
    dht::DHT,
    handler::Handler,
    msg::msg,
    task::{
        Task, TaskData,
        ClosestSet,
        CandidateNode,
    }
};

const MAX_TODO_ENTRIES: usize = 24;

pub(crate) struct ValueAnnounceTask {
    base_data: TaskData,

    todo: Rc<RefCell<VecDeque<Rc<RefCell<CandidateNode>>>>>,
    value: Value,
    expected_seq: i32,

    dht: Rc<RefCell<DHT>>
}

impl ValueAnnounceTask {
    pub(crate) fn new(
        dht: Rc<RefCell<DHT>>,
        value: Value,
        expected_seq: i32,
    ) -> Self {
        Self {
            base_data: TaskData::new(),
            todo: Rc::new(RefCell::new(
                VecDeque::with_capacity(MAX_TODO_ENTRIES))),
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

    fn dht(&self) -> Rc<RefCell<DHT>> {
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
            let msg = msg::store_value_request(
                self.value.clone(),
                token,
                self.expected_seq,
            );

            let cloned_todo = self.todo.clone();
            let handler = Handler::new(move |_| {
                cloned_todo.borrow_mut().pop_front();
            });

            self.send_call(cn.into(), msg, Some(handler));
        }
    }

    fn is_done(&self) -> bool {
        self.todo.borrow().is_empty() &&
            self.data().is_done()
    }
}
