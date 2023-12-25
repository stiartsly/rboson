use std::rc::Rc;
use std::cell::RefCell;
use std::time::SystemTime;
use std::time::Duration;
use libsodium_sys::randombytes_uniform;

use crate::{
    unwrap,
    constants,
    Id,
    NodeInfo,
    dht::DHT,
    scheduler::Scheduler,
    msg::msg::{self, Msg}
};

#[derive(Clone, PartialEq, PartialOrd, Debug)]
pub(crate) enum State {
    Unsent,
    Sent,
    Stalled,
    Timeout,
    Err,
    Responsed,
}

pub(crate) struct RpcCall {
    txid: i32,
    target: Rc<NodeInfo>,

    req: Option<Rc<RefCell<Box<dyn Msg>>>>,
    rsp: Option<Rc<RefCell<Box<dyn Msg>>>>,

    sent: SystemTime,
    responsed: SystemTime,

    state: State,

    state_changed_fn: Box<dyn Fn(&RpcCall, &State, &State)>,
    responsed_fn: Box<dyn Fn(&RpcCall, Rc<RefCell<Box<dyn Msg>>>)>,
    stalled_fn: Box<dyn Fn(&RpcCall)>,
    timeout_fn: Box<dyn Fn(&RpcCall)>,

    dht: Rc<RefCell<DHT>>,
    scheduler: Option<Rc<RefCell<Scheduler>>>,
    cloned: Option<Rc<RefCell<RpcCall>>>,
}

static mut NEXT_TXID: i32 = 0;
fn next_txid() -> i32 {
    unsafe {
        if NEXT_TXID == 0 {
            NEXT_TXID = randombytes_uniform(u32::MAX) as i32
        }

        NEXT_TXID += 1;
        if NEXT_TXID == 0 {
            NEXT_TXID += 1;
        }
        NEXT_TXID
    }
}

impl RpcCall {
    pub(crate) fn new(target: Rc<NodeInfo>,
        dht: Rc<RefCell<DHT>>,
        msg: Rc<RefCell<Box<dyn Msg>>>) -> Self
    {
        msg.borrow_mut().set_remote(
            target.id(),
            target.socket_addr()
        );

        RpcCall {
            txid: next_txid(),
            target,
            req: Some(msg),
            rsp: None,

            sent: SystemTime::UNIX_EPOCH,
            responsed: SystemTime::UNIX_EPOCH,
            state: State::Unsent,

            state_changed_fn: Box::new(|_, _, _| {}),
            responsed_fn: Box::new(|_, _| {}),
            stalled_fn: Box::new(|_| {}),
            timeout_fn: Box::new(|_| {}),

            dht,
            scheduler: None,
            cloned: None,
        }
    }

    pub(crate) fn txid(&self) -> i32 {
        self.txid
    }

    pub(crate) fn dht(&self) -> Rc<RefCell<DHT>> {
        self.dht.clone()
    }

    pub(crate) fn target_id(&self) -> &Id {
        self.target.id()
    }

    pub(crate) fn target(&self) -> Rc<NodeInfo> {
        self.target.clone()
    }

    pub(crate) fn set_cloned(&mut self, cloned: Rc<RefCell<RpcCall>>) {
        self.cloned = Some(cloned);
    }

    pub(crate) fn matches_id(&self) -> bool {
        self.rsp.as_ref().and_then(|rsp| {
            Some(rsp.borrow().id() == self.target_id())
        }).unwrap_or(false)
    }

    pub(crate) fn matches_addr(&self) -> bool {
        self.req.as_ref().and_then(|req| {
            self.rsp.as_ref().map(|rsp| {
                rsp.borrow().origin() == req.borrow().remote_addr()
            })
        }).unwrap_or(false)
    }

    pub(crate) fn req(&self) ->Option<Rc<RefCell<Box<dyn Msg>>>> {
        self.req.as_ref().cloned()
    }

    pub(crate) fn rsp(&self) -> Option<Rc<RefCell<Box<dyn Msg>>>>  {
        self.rsp.as_ref().cloned()
    }

    pub(crate) fn sent_time(&self) -> &SystemTime {
        &self.sent
    }

    pub(crate) fn set_state_changed_fn<F>(&mut self, f: F)
    where F: Fn(&RpcCall, &State, &State) + 'static {
        self.state_changed_fn = Box::new(f)
    }

    pub(crate) fn set_responsed_fn<F>(&mut self, f: F)
    where F: Fn(&RpcCall, Rc<RefCell<Box<dyn Msg>>>) + 'static {
        self.responsed_fn = Box::new(f)
    }

    pub(crate) fn set_stalled_fn<F>(&mut self, f: F)
    where F: Fn(&RpcCall) + 'static {
        self.stalled_fn = Box::new(f)
    }

    pub(crate) fn set_timeout_fn<F>(&mut self, f: F)
    where F: Fn(&RpcCall) + 'static {
        self.timeout_fn = Box::new(f)
    }

    pub(crate) fn update_state(&mut self, new_state: State) {
        let prev_state = self.state.clone();
        self.state = new_state;

        (self.state_changed_fn)(self, &prev_state, &self.state);
        match self.state {
            State::Timeout => (self.timeout_fn)(self),
            State::Stalled => (self.stalled_fn)(self),
            State::Responsed => {
                if let Some(rsp) = self.rsp() {
                    (self.responsed_fn)(self, rsp)
                }
            }
            _ => {}
        }
    }

    pub(crate) fn send(&mut self, scheduler: Rc<RefCell<Scheduler>>) {
        self.sent = SystemTime::now();
        self.update_state(State::Sent);

        self.scheduler = Some(scheduler);
        let cloned = self.cloned.as_ref().unwrap().clone();
        unwrap!(self.scheduler).borrow_mut().add_oneshot(move || {
            cloned.borrow_mut().check_timeout();
        }, 2*1000);
    }

    pub(crate) fn responsed(&mut self, msg: Rc<RefCell<Box<dyn Msg>>>) {
        self.rsp = Some(msg.clone());
        self.responsed = SystemTime::now();

        match msg.borrow().kind() {
            msg::Kind::Request  => {},
            msg::Kind::Response => self.update_state(State::Responsed),
            msg::Kind::Error    => self.update_state(State::Err)
        }
    }

    pub(crate) fn responsed_socket_mismatch(&mut self) {}

    pub(crate) fn stall(&mut self) {
        if self.state != State::Sent {
            self.update_state(State::Stalled)
        }
    }

    pub(crate) fn check_timeout(&mut self) {
        if self.state != State::Sent && self.state != State::Stalled {
            return;
        }

        let timeout = Duration::from_millis(constants::RPC_CALL_TIMEOUT_MAX);
        let elapsed = self.sent.elapsed().unwrap();

        if timeout > elapsed {
            let remaining = (timeout - elapsed).as_millis() as u64;

            self.update_state(State::Stalled);
            let cloned = self.cloned.as_ref().unwrap().clone();
            self.scheduler.as_ref().unwrap().borrow_mut().add_oneshot(move || {
                cloned.borrow_mut().check_timeout()
            }, remaining);

        } else {
            self.update_state(State::Timeout);
        }
    }
}
