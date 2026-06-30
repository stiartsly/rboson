use std::{
    fmt,
    any::Any,
    rc::{Rc, Weak},
    cell::RefCell,
    collections::HashSet,
    sync::atomic::{Ordering, AtomicI32},
};
use log::{warn, debug};
use crate::core::Network;
use crate::dht::{
    dht::DHT,
    msg::Message,
    handler::Handler,
    task::task_listener::TaskListener,
    rpc::{
        Target, RpcCall, rpccall,
        listener::Listener
    },
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
    state       : State,

    //created     : SystemTime,
    //started     : SystemTime,
    //ended       : SystemTime,

    inflights   : HashSet<i32>,
    listener    : Option<TaskListener>,
    end_handler : Option<Handler<()>>,

    nested      : RefCell<Option<Box<dyn Task>>>,
    cloned      : Option<Weak<RefCell<Box<dyn Task>>>>,
}

impl TaskData {
    pub(crate) fn new() -> Self {
        Self {
            taskid      : next_taskid(),
            task_name   : String::new(),
            state       : State::Initialized,
            inflights   : HashSet::new(),
            listener    : None,
            end_handler : None,
            nested      : RefCell::new(None),
            cloned      : None,

        }
    }

    pub(crate) fn is_done(&self) -> bool {
        self.inflights.is_empty()
    }
}

pub(crate) trait Task {
    fn data(&self) -> &TaskData;
    fn data_mut(&mut self) -> &mut TaskData;

    fn as_task(&self) -> &dyn Task;
    fn as_any(&self) -> &dyn Any;

    fn dht(&self) -> Rc<RefCell<DHT>>;

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
        *self.data_mut().nested.borrow_mut() = Some(nested);
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
        self.data().nested.borrow_mut().take()
    }

    fn inflight_size(&self) -> usize {
        self.data().inflights.len()
    }

    fn with_ended_handler(&mut self, handler: Handler<()>) {
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

    fn set_cloned(&mut self, weak: Weak<RefCell<Box<dyn Task>>>) {
        self.data_mut().cloned = Some(weak);
    }

    fn cloned(&self) -> Weak<RefCell<Box<dyn Task>>> {
        self.data().cloned.clone().expect("Task instance is dropped")
    }

    fn start(&mut self) {
        if self.set_state_if_stateset(&UNSTARTED_STATES, State::Running) {
            debug!("{}#{} starting...",
                self.task_name(),
                self.task_id()
            );
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

        // Cancel nested one.
        {
            let mut nested = self.data_mut().nested.borrow_mut();
            if let Some(nested) = nested.as_mut() {
                nested.cancel();
            }
        }

        debug!("Task {}#{} canceled",
            self.task_name(),
            self.task_id()
        );

        let handler = self.data_mut().end_handler.as_mut();
        if let Some(ended) = handler {
            ended.cb(&());
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

        debug!("Task {}#{} completed",
            self.task_name(),
            self.task_id()
        );

        let handler = self.data_mut().end_handler.as_mut();
        if let Some(ended) = handler {
            ended.cb(&());
        }

        let listener = self.data_mut().listener.take();
        if let Some(l) = listener {
            l.ended(self.as_task());
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
/*
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
*/
    fn can_dorequest(&self) -> bool {
        self.is_running() &&
            self.inflight_size() < 16
    }

    fn prepare(&mut self) {}
    fn iterate(&mut self) {}

    fn call_sent(&mut self, _: &RpcCall) {}
    fn call_responded(&mut self, _: &RpcCall) {}
    fn call_error(&mut self, _: &RpcCall) {}
    fn call_timeout(&mut self, _: &RpcCall) {}

    fn send_call(&mut self, target: Target, msg: Message, handler: Option<Handler<()>>) {
        if !self.can_dorequest() {
            return;
        }

        let task = self.cloned().upgrade().expect("Task instance is dropped");
        let listener = Listener::new(move |c, _, state| {
            if task.borrow().is_ended() {
                debug!("{}#{} call to {} state changed ignored due to the task is terminated",
                    task.borrow().task_name(),
                    task.borrow().task_id(),
                    c.target_id());
                return;
            }

            let mut task = task.borrow_mut();
            match state {
                rpccall::State::Sent => task.call_sent(c),
                rpccall::State::Responded => {
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_ended() && c.rsp().is_some() {
                        task.call_responded(c);
                    }
                },
                rpccall::State::Err => {
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_ended() {
                        task.call_error(c);
                    }
                },
                rpccall::State::Timeout => {
                    task.data_mut().inflights.remove(&c.txid());
                    if !task.is_ended() {
                        task.call_timeout(c);
                    }
                },
                _ => {},
            }

            if state >= rpccall::State::Stalled {
                task.try_iterate();
            }
        });

        let mut call = RpcCall::new(target, msg);
        call.set_listener(listener);

        handler.map(|v| v.cb(&()));
        self.data_mut().inflights.insert(call.txid());

        let dht = self.dht();
        let _ = tokio::task::spawn_local(async move {
            let rs = dht.borrow().rs();
            let _ = rs.borrow_mut()
                .send_call(call)
                .map_err(|e| log::error!("{e}"));
        });
    }

    fn network(&self) -> Network {
        self.dht().borrow().network()
    }
}

impl fmt::Display for dyn Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let network = self.network();

        write!(f,
            "#{}[{}] DHT:{}, state:{}",
            self.task_id(),
            self.task_name(),
            network,
            self.task_state()
        )
    }
}
