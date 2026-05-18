use std::fmt;
use std::sync::{Arc, Mutex};
use std::any::Any;
use std::time::{SystemTime, Duration};
use std::collections::HashMap;
use log::{warn,debug, trace};

use crate::{
    addr_family,
    core::Result,
    NodeInfo,
    PeerInfo,
    Value,
};

use crate::dht::{
    rpccall::{RpcCall, State as CallState},
    node_entry::NodeEntry,
    dht::DHT,
    msg::msg::Message,
    task::task_listener::TaskListener,
    task::closest_set::ClosestSet,
};

pub(crate) enum TaskResult {
    NodeInfo(NodeInfo),
    PeerInfo(Vec<PeerInfo>),
    Value(Value),
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Initial,
    Queued,
    Running,
    Completed,
    Canceled,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            State::Initial => "INITIAL",
            State::Queued => "QUEUED",
            State::Running => "RUNNING",
            State::Completed => "COMPLETED",
            State::Canceled => "CANCELED",
        })
    }
}

pub(crate) type TaskId = i32;
static mut NEXT_TASKID: TaskId= 0;

fn next_taskid() -> TaskId {
    unsafe {
        NEXT_TASKID += 1;
        if NEXT_TASKID == 0 {
            NEXT_TASKID += 1;
        }
        NEXT_TASKID
    }
}

pub(crate) struct TaskData {
    taskid  : TaskId,
    name    : String,
    state   : State,

    created_time    : SystemTime,
    started_time    : SystemTime,
    end_time        : SystemTime,

    inflights: HashMap<TaskId, Arc<Mutex<RpcCall>>>,
    //listeners: Vec<Box<dyn TaskListener>>,

   // ended_fn: Box<dyn Fn(Box<dyn Task>)>,
    ended_fn: Option<Box<dyn Fn(&dyn Task)>>,

    nested: Option<Arc<Mutex<Box<dyn Task>>>>,
    cloned: Option<Arc<Mutex<Box<dyn Task>>>>,

    dht: Arc<Mutex<DHT>>,
}

impl TaskData {
    pub(crate) fn new(dht: Arc<Mutex<DHT>>) -> Self {
        Self {
            taskid: next_taskid(),
            name: String::new(),
            state: State::Initial,

            created_time    : SystemTime::now(),
            started_time    : SystemTime::UNIX_EPOCH,
            end_time        : SystemTime::UNIX_EPOCH,

            inflights: HashMap::new(),
            //listeners: Vec::new(),

            ended_fn: None,

            nested: None,
            cloned: None,
            dht,
        }
    }

    pub(crate) fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
    }

    pub(crate) fn is_done(&self) -> bool {
        self.inflights.is_empty()
    }
}

pub(crate) trait Task: Any + Send {
    fn data(&self) -> &TaskData;
    fn data_mut(&mut self) -> &mut TaskData;

    fn result(&self) -> Option<TaskResult> {
        None
    }

    fn set_cloned(&mut self, task: Arc<Mutex<Box<dyn Task>>>) {
        self.data_mut().cloned = Some(task);
    }

    fn cloned(&self) -> Arc<Mutex<Box<dyn Task>>> {
        self.data().cloned.as_ref()
            .expect("panic: self cloned not set, this should never happen")
            .clone()
    }

    fn id(&self) -> i32 {
        self.data().taskid
    }

    fn set_name(&mut self, name: String) {
        self.data_mut().name = name;
    }

    fn name(&self) -> &str {
        self.data().name.as_str()
    }

    fn check_state_and_set(&mut self, expected: State, new_state: State) -> bool {
        if expected != self.state() {
            warn!("{}#{} invalid state transition: expected {}, but was {}",
                    self.name(), self.id(), expected, self.state());
            return false;
        }

        if self.is_end() {
            warn!("{}#{} invalid state transition: task already ended: {}",
                    self.name(), self.id(), self.state());
            return false;
        }

        self.data_mut().state = new_state;
        true
    }

    fn check_stateset_and_set(&mut self, expected: &[State], new_state: State) -> bool {
        if !expected.contains(&self.state()) {
            let expected_str = expected.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            warn!("{}#{} invalid state transition: expected one of {}, but was {}",
                    self.name(), self.id(), expected_str, self.state());
            return false;
        }

        self.data_mut().state = new_state;
        true
    }

    fn state(&self) -> State {
        self.data().state
    }

    fn set_nested(&mut self, nested: Arc<Mutex<Box<dyn Task>>>) {
        self.data_mut().nested = Some(nested);
    }

    fn nested(&self) -> Option<Arc<Mutex<Box<dyn Task>>>> {
        self.data().nested.as_ref().cloned()
    }

    fn inflight_size(&self) -> usize {
        self.data().inflights.len()
    }

    fn add_ended_fn(&mut self, f: Box<dyn Fn(&dyn Task)>) {
        self.data_mut().ended_fn = Some(f);
    }

    fn start(&mut self) {
        /*
        if self.check_stateset_and_set(&[State::Initial, State::Queued], State::Running) {
            debug!("{}#{} starting...", self.name(), self.id());
            self.data_mut().started_time = SystemTime::now();

            self.prepare();
            for listener in &self.data().listeners {
                listener.started();
            }

            if let Err(e) = self.try_iterate() {
                warn!("{}#{} start failed: {}",
                    self.name(), self.id(), e);
            }
        }*/
        unimplemented!()
    }

    fn try_iterate(&mut self) -> Result<()> {
        if self.is_done() {
            self.complete();
            return Ok(())
        }

        if self.can_dorequest() && !self.is_end() {
            self.iterate();

            // Check again in case todo-queue has been drained by update()
			if self.is_done() {
                self.complete();
                return Ok(())
            }
        }
        Ok(())
    }

    fn cancel(&mut self) {
        /*
        let incompleted = vec![
            State::Initial,
            State::Queued,
            State::Running
        ];
        if self.check_stateset_and_set(&incompleted, State::Canceled) {
            self.data_mut().end_time = SystemTime::now();

            if let Some(nested) = self.data_mut().nested.as_mut() {
                nested.lock().unwrap().cancel()
            }

            debug!("{}#{} canceled",
                self.name(),
                self.id()
            );

            // TODO: endHandler

            for listener in &self.data().listeners {
                listener.cancelled();
                listener.ended();
            }
        }
        */
        unimplemented!()
    }

    fn complete(&mut self) {
        /*
        let incompleted = vec![
            State::Initial,
            State::Queued,
            State::Running
        ];

        if self.check_stateset_and_set(&incompleted, State::Completed) {
            self.data_mut().end_time = SystemTime::now();

            debug!("{}#{} completed",
                self.name(),
                self.id()
            );

            // TODO: endHandler

            for listener in &self.data().listeners {
                listener.completed();
                listener.ended();
            }
        }
        */
        unimplemented!()
    }

    fn is_unstarted(&self) -> bool {
        self.data().state == State::Initial ||
        self.data().state == State::Queued
    }

    fn is_running(&self) -> bool {
        self.data().state == State::Running
    }

    fn is_complete(&self) -> bool {
        self.data().state == State::Completed
    }

    fn is_canceled(&self) -> bool {
        self.data().state == State::Canceled
    }

    fn is_end(&self) -> bool {
        self.data().state == State::Completed ||
        self.data().state == State::Canceled
    }

    fn is_done(&self) -> bool {
        self.data().is_done()
    }

    fn started_time(&self) -> SystemTime {
        self.data().started_time
    }

    fn end_time(&self) -> SystemTime {
        self.data().end_time
    }

    fn leading_time(&self) -> Option<Duration> {
        // TODO:
        None
    }

    fn age(&self) -> Option<Duration> {
        // TODO:
        None
    }

    fn can_dorequest(&self) -> bool {
        // TODO:
        true
    }


    fn prepare(&mut self) {}
    fn iterate(&mut self) {}

    fn call_sent(&mut self, _: &RpcCall) {}
    fn call_responsed(&mut self, _: &RpcCall) {}
    fn call_error(&mut self, _: &RpcCall) {}
    fn call_timeout(&mut self, _: &RpcCall) {}

    fn send_call(&mut self,
        _ni: NodeEntry,
        _msg: Arc<Mutex<Message>>,
        _cb: Box<dyn FnMut(Arc<Mutex<RpcCall>>)>)
        -> Result<()> {

        // TODO:
        Ok(())
    }

/*
    fn send_call(&mut self,
        ni: NodeInfo,
        msg: Arc<Mutex<Message>>,
        mut cb: Box<dyn FnMut(Arc<Mutex<RpcCall>>)>)
    -> Result<(), Error> {
        if !self.can_request() {
            return Ok(())
        }

        let call = RpcCall::new_shared(
            NodeEntry::NodeEntry(ni),
             self.data().dht(),
             msg
        );
        let task = self.data().task();

        call.lock().unwrap().set_cloned(call.clone());
        call.lock().unwrap().set_state_changed_fn (move|c, _, cur| {
            let mut task = task.lock().unwrap();
            let mut update_needed = true;
            match cur {
                CallState::Sent => task.call_sent(c),
                CallState::Responsed => {
                    update_needed = true;
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_finished() && c.rsp().is_some() {
                        task.call_responsed(c, c.rsp().as_ref().unwrap().clone());
                    }
                },
                CallState::Err => {
                    update_needed = true;
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_finished() {
                        task.call_error(c);
                    }

                },
                CallState::Timeout => {
                    update_needed = true;
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_finished() {
                        task.call_timeout(c);
                    }
                }
                CallState::Stalled => {
                    update_needed = true;
                }
                _ => {}
            }

            if update_needed && task.is_done() {
                task.finish();
            }
        });

        (cb)(call.clone());
        self.data_mut().inflights.insert(call.borrow().txid(), call.clone());

        trace!("Task#{} sending call to {}@{}",
            self.taskid(),
            self.data().dht.borrow().id(),
            self.data().dht.borrow().addr()
        );

        self.data().dht.borrow()
            .server()
            .borrow_mut()
            .send_call(call);

        Ok(())
    }
    */

    fn closest(&self) -> Option<&ClosestSet> {
        unimplemented!()
    }

    fn with_closest(&mut self, _closest: ClosestSet) {
        unimplemented!()
    }
}

impl fmt::Display for dyn Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "#{}[{}] DHT:{}, state:{}",
            self.id(),
            self.name(),
            addr_family!(self.data().dht().lock().unwrap().addr()),
            self.state()
        )?;
        Ok(())
    }
}
