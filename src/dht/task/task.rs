use std::{
    fmt,
    any::Any,
    sync::{Arc, Weak, Mutex},
    sync::atomic::{Ordering, AtomicI32},
    collections::HashSet,
    time::{SystemTime, Duration}
};
use log::{warn, debug};

use crate::core::Result;
use crate::dht::{
    dht::DHT,
    msg::Message,
    consumer::Consumer,
    task::task_listener::TaskListener,
    rpc::{
        Target,
        RpcCall, rpccall::State as CallState,
    }
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub(crate) enum State {
    Initialized,
    Queued,
    Running,
    Completed,
    Canceled,
}

const UNSTARTED_STATES: [State; 2] = [
    State::Initialized,
    State::Queued
];
const INCOMPLETED_STATES: [State; 3] = [
    State::Initialized,
    State::Queued,
    State::Running
];

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            State::Initialized => "INITIAL",
            State::Queued => "QUEUED",
            State::Running => "RUNNING",
            State::Completed => "COMPLETED",
            State::Canceled => "CANCELED",
        })
    }
}

/** Maximum concurrent RPC requests for normal tasks. */
const MAX_CONCURRENT_RPC_REQUESTS: usize = 16;
/** Maximum concurrent RPC requests for low-priority tasks. */
const MAX_CONCURRENT_RPC_REQUESTS_LOW_PRIORITY: usize = 4;

pub(crate) type TaskId = i32;
static NEXT_TASKID: AtomicI32 = AtomicI32::new(0);
fn next_taskid() -> TaskId {
    let id = NEXT_TASKID.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    if id == 0 {
        NEXT_TASKID.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    } else {
        id
    }
}

pub(crate) struct TaskData {
    taskid      : TaskId,
    task_name   : String,
    low_priori  : bool,
    state       : State,

    created     : SystemTime,
    started     : SystemTime,
    ended       : SystemTime,

    inflights   : HashSet<i32>,
    listener    : Option<TaskListener>,
    end_handler : Option<Consumer<()>>,

    nested      : Mutex<Option<Box<dyn Task>>>
}

impl TaskData {
    pub(crate) fn new() -> Self {
        Self {
            taskid      : next_taskid(),
            task_name   : String::new(),
            low_priori  : false,
            state       : State::Initialized,
            created     : SystemTime::now(),
            started     : SystemTime::UNIX_EPOCH,
            ended       : SystemTime::UNIX_EPOCH,
            inflights   : HashSet::new(),
            listener    : None,
            end_handler : None,
            nested      : Mutex::new(None),
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        self.inflights.is_empty()
    }
}

pub(crate) trait Task: Send + Sync {
    fn data(&self) -> &TaskData;
    fn data_mut(&mut self) -> &mut TaskData;

    fn as_task(&self) -> &dyn Task;
    fn as_any(&self) -> &dyn Any;

    fn dht(&self) -> Weak<Mutex<DHT>>;

    fn task_id(&self) -> i32 {
        self.data().taskid
    }
    fn task_name(&self) -> &str {
        self.data().task_name.as_str()
    }
    fn task_state(&self) -> State {
        self.data().state
    }

    fn with_name(&mut self, name: String) {
        self.data_mut().task_name = name;
    }

    fn with_nested(&mut self, nested: Box<dyn Task>) {
        *self.data_mut().nested.lock().unwrap() = Some(nested);
    }

    fn set_state_if(&mut self, expected: &State, new_state: State) -> bool {
        if expected != &self.task_state() {
            warn!("{}#{} invalid state transition: expected {}, but was {}",
                self.task_name(),
                self.task_id(),
                expected,
                self.task_state()
            );
            return false;
        }

        if self.is_ended() {
            warn!("{}#{} invalid state transition: task already ended: {}",
                self.task_name(),
                self.task_id(),
                self.task_state()
            );
            return false;
        }

        self.data_mut().state = new_state;
        true
    }

    fn set_state_if_stateset(&mut self, expected: &[State], new_state: State) -> bool {
        if !expected.contains(&self.task_state()) {
            let str = expected.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            warn!("{}#{} invalid state transition: expected one of {}, but was {}",
                self.task_name(),
                self.task_id(),
                str,
                self.task_state()
            );
            return false;
        }

        self.data_mut().state = new_state;
        true
    }

    fn nested(&self) -> Option<Box<dyn Task>> {
        self.data().nested.lock().unwrap().take()
    }

    fn inflight_size(&self) -> usize {
        self.data().inflights.len()
    }

    fn with_ended_handler(&mut self, handler: Consumer<()>) {
        self.data_mut().end_handler = Some(handler);
    }

    fn with_listener(&mut self, listener: TaskListener) {
        self.data_mut().listener = Some(listener);

        let Some(l) = self.data_mut().listener.take() else {
            return;
        };

        let task = self.as_task();
        if self.is_canceled() {
            l.canceled(task);
            l.ended(task);
        } else if self.is_completed() {
            l.completed(task    );
            l.ended(task);
        } else {}

        self.data_mut().listener = Some(l);
    }

    fn cloned(&self) -> Arc<Mutex<Box<dyn Task>>> {
        unimplemented!()
    }

    fn start(&mut self) {
        if self.set_state_if_stateset(&UNSTARTED_STATES, State::Running) {
            debug!("{}#{} starting...",
                self.task_name(),
                self.task_id()
            );
            self.data_mut().started = SystemTime::now();

            self.prepare();

            let listener = self.data_mut().listener.take();
            if let Some(l) = listener {
                l.started(self.as_task());
                self.data_mut().listener = Some(l);
            }

            self.try_iterate();
        }
    }

    fn try_iterate(&mut self) {
        if self.is_done() {
            self.complete();
            return;
        }

        if self.can_dorequest() && !self.is_ended() {
            self.iterate();

            // Check again in case todo-queue has been drained by update()
			if self.is_done() {
                self.complete();
            }
        }
    }

    fn cancel(&mut self) {
        if !self.set_state_if_stateset(&INCOMPLETED_STATES, State::Canceled) {
            return;
        }

        // Ended nested one.
        {
            let mut nested = self.data_mut().nested.lock().unwrap();
            if let Some(nested) = nested.as_mut() {
                nested.cancel();
            }
        }

        self.data_mut().ended = SystemTime::now();
        debug!("Task {}#{} canceled",
            self.task_name(),
            self.task_id()
        );

        let handler = self.data_mut().end_handler.as_mut();
        if let Some(ended) = handler {
            ended.accept(());
        }

        let listener = self.data_mut().listener.take();
        if let Some(l) = listener {
            l.canceled(self.as_task());
            l.ended(self.as_task());
            self.data_mut().listener = Some(l);
        }
    }

    fn complete(&mut self) {
        if !self.set_state_if_stateset(&INCOMPLETED_STATES, State::Completed){
            return;
        }

        self.data_mut().ended = SystemTime::now();
        debug!("Task {}#{} completed",
            self.task_name(),
            self.task_id()
        );

        let handler = self.data_mut().end_handler.as_mut();
        if let Some(ended) = handler {
            ended.accept(());
        }

        let listener = self.data_mut().listener.take();
        if let Some(l) = listener {
            l.canceled(self.as_task());
            self.data_mut().listener = Some(l);
        }
    }

    fn is_unstarted(&self) -> bool {
        self.data().state == State::Initialized ||
        self.data().state == State::Queued
    }

    fn is_running(&self) -> bool {
        self.data().state == State::Running
    }

    fn is_completed(&self) -> bool {
        self.data().state == State::Completed
    }

    fn is_canceled(&self) -> bool {
        self.data().state == State::Canceled
    }

    fn is_ended(&self) -> bool {
        self.is_completed() || self.is_canceled()
    }

    fn is_done(&self) -> bool {
        self.data().inflights.is_empty()
    }

    #[allow(unused)]
    fn started_time(&self) -> SystemTime {
        self.data().started
    }

    #[allow(unused)]
    fn ended_time(&self) -> SystemTime {
        self.data().ended
    }

    #[allow(unused)]
    fn leading_time(&self) -> Option<Duration> {
        self.data().ended.duration_since(
            self.data().started
        ).ok()
    }

    #[allow(unused)]
    fn age(&self) -> Option<Duration> {
        self.data().created.elapsed().ok()
    }

    fn can_dorequest(&self) -> bool {
        let limit = if self.data().low_priori {
            MAX_CONCURRENT_RPC_REQUESTS_LOW_PRIORITY
        } else {
            MAX_CONCURRENT_RPC_REQUESTS
        };

        self.is_running() &&
            self.inflight_size() < limit
    }

    fn prepare(&mut self) {}
    fn iterate(&mut self) {}

    fn call_sent(&mut self, _: &RpcCall) {}
    fn call_responded(&mut self, _: &RpcCall) {}
    fn call_error(&mut self, _: &RpcCall) {}
    fn call_timeout(&mut self, _: &RpcCall) {}

    fn send_call(&mut self, target: Target, msg: Message,
        consumer: Option<Consumer<()>>) -> Result<()> {

        if !self.can_dorequest() {
            return Ok(())
        }

        let mut call = RpcCall::new(target, msg);
        let task = self.cloned();
        call.set_simple_listener(move |c, _, state| {
            if task.lock().unwrap().is_ended() {
                debug!("{}#{} call to {} state changed ignored due to the task is terminated",
                    task.lock().unwrap().task_name(),
                    task.lock().unwrap().task_id(),
                    c.target_id());
                return;
            }

            let mut task = task.lock().unwrap();
            match state {
                CallState::Sent => task.call_sent(c),
                CallState::Responded => {
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_ended() && c.rsp().is_some() {
                        task.call_responded(c);
                    }
                },
                CallState::Err => {
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_ended() {
                        task.call_error(c);
                    }
                },
                CallState::Timeout => {
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_ended() {
                        task.call_timeout(c);
                    }
                },
                _ => {},
            }

            if state >= CallState::Stalled {
                task.try_iterate();
            }
        });

        if let Some(handler) = consumer {
            handler.accept(());
        };

        let txid = call.txid();
        self.data_mut().inflights.insert(txid);

        let dht = self.dht().upgrade()
            .expect("panic: DHT instance dropped.");
        let server = dht.lock().unwrap().server().clone();
        let _ = server.lock().unwrap().send_call(call);
        Ok(())
    }
}

impl fmt::Display for dyn Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dht = self.dht().upgrade()
            .expect("panic: DHT instance dropped.");
        let addr = dht.lock().unwrap().addr().clone();
        let addr_family = match addr.is_ipv4() {
            true => "ipv4",
            false => "ipv6"
        };

        write!(f,
            "#{}[{}] DHT:{}, state:{}",
            self.task_id(),
            self.task_name(),
            addr_family,
            self.task_state()
        )
    }
}
