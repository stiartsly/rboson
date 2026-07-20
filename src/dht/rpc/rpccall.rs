use std::{
    rc::{Rc, Weak},
    cell::RefCell,
    time::SystemTime
};
use log::error;
use crate::Id;
use crate::dht::{
    msg::{Message, msg::Kind},
    timer_client::LocalTimerClient as TimerClient,
    handler::LocalHandler as AsyncHandler,
    handler::Handler,
    rpc::{
        Target,
        Listener as CallListener,
    },
};

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
#[repr(u32)]
pub(crate) enum State {
    Unsent,     // Call has not been sent yet
    Sent,       // Call has been sent, awaiting response
    Stalled,    // Call is delayed, possibly due to network issues
    Timeout,    // Call timed out without a response
    // Canceled,   // Call was canceled before completion
    Err,        // Call failed due to an error
    Responded,  // Call received a valid response
}

impl State {
    pub(crate) fn is_final(&self) -> bool {
        self >= &State::Timeout
    }
}

pub(crate) struct RpcCall {
    txid            : i32,
    target          : Target,

    // a transient field to store request message before
    // being nailed as Arc<Message> object.
    transient       : Option<Message>,

    req             : Option<Rc<Message>>,
    rsp             : Option<Rc<Message>>,

    sent_time       : Option<SystemTime>,
    rsp_time        : Option<SystemTime>,

    state           : State,

    listener        : Option<CallListener>,

    timer_id        : Option<u64>,
    timer_client    : Option<Rc<TimerClient>>,
    timeout_handler : Option<Handler<()>>,

    cloned          : Weak<RefCell<Self>>,
}

impl RpcCall {
    pub(crate) fn new(target: impl Into<Target>, mut req: Message) -> Self {
        let target: Target = target.into();
        req.set_remote(target.id(),target.socket_addr());

        Self {
            txid            : req.txid(),
            target,
            transient       : Some(req),
            req             : None,
            rsp             : None,
            sent_time       : None,
            rsp_time        : None,
            state           : State::Unsent,
            listener        : None,
            timer_id        : None,
            timer_client    : None,
            timeout_handler : None,

            cloned          : Weak::new(),
        }
    }

    pub(crate) fn set_cloned(&mut self, cloned: Weak<RefCell<RpcCall>>) {
        self.cloned = cloned;
    }

    pub(crate) fn target_id(&self) -> Id {
        self.target.id()
    }

    pub(crate) fn target(&self) -> &Target {
        &self.target
    }

    pub(crate) fn take_transient(&mut self) -> Message {
        self.transient.take().expect("Transient message not set")
    }

    pub(crate) fn set_request(&mut self, req: Rc<Message>) {
        self.req = Some(req);
    }

    //pub(crate) fn is_reachable_at_creation(&self) -> bool {
    //    self.target_reachable
    //}

    pub(crate) fn txid(&self) -> i32 {
        self.txid
    }

    pub(crate) fn req(&self) -> Rc<Message> {
        self.req.as_ref().expect("Request not set").clone()
    }
    pub(crate) fn rsp(&self) -> Option<Rc<Message>> {
        self.rsp.as_ref().cloned()
    }

    pub(crate) fn nodeid_mismatched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.nodeid() != &self.target_id()
        }).unwrap_or(false)
    }

    pub(crate) fn addr_mismatched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.remote_addr() != self.req().remote_addr()
        }).unwrap_or(false)
    }

    pub(crate) fn sent_time(&self) -> Option<SystemTime> {
        self.sent_time
    }

    pub(crate) fn set_listener(&mut self, listener: CallListener) {
        if self.state != State::Unsent {
            return;
        }
        self.listener = Some(listener);
    }

    pub(crate) fn set_timeout_handler(&mut self, handler: Handler<()>) {
        self.timeout_handler = Some(handler);
    }

    pub(crate) fn set_timer_client(&mut self, timer_client: Rc<TimerClient>) {
        self.timer_client = Some(timer_client);
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> State {
        self.state
    }

    pub(crate) fn update_state(&mut self, new_state: State) {
        let prev = self.state;
        self.state = new_state;

        if new_state == State::Timeout {
            if let Some(h) = self.timeout_handler.take() {
                h.cb(&());
            }
        }

        let Some(l) = self.listener.take() else {
            return;
        };

        l.on_state_change(self, prev, new_state);
        match new_state {
            State::Responded => l.on_response(self),
            State::Stalled => l.on_stall(self),
            State::Timeout => l.on_timeout(self),
            _ => {}
        }
        self.listener = Some(l);
    }

    fn set_timeout_timer(&mut self, timeout: u64)
    {
        let Some(timer_client) = self.timer_client.clone() else {
            error!("Timer client not set for RpcCall");
            return;
        };

        let cloned = self.cloned.upgrade().expect("RpcCall weak reference not set");
        let result = timer_client.add_timer(timeout, None,
            AsyncHandler::new(move |_| {
                let cloned = cloned.clone();
                Box::pin(async move {
                    cloned.borrow_mut().check_timeout();
                })
        }));
        let Ok(timer_id) = result else {
            error!("Failed to set timeout timer: {}", result.unwrap_err());
            return;
        };

        self.timer_id = Some(timer_id);
    }

    fn cancel_timeout_timer(&mut self) {
        let Some(timer_client) = self.timer_client.take() else {
            return;
        };
        let Some(timer_id) = self.timer_id.take() else {
            return;
        };
        let _ = timer_client.cancel_timer(timer_id);
    }

    fn check_timeout(&mut self) {
        if self.state != State::Sent && self.state != State::Stalled {
            return;
        }
        self.update_state(State::Timeout);

        self.timer_id = None;
        self.timer_client = None;
    }

    pub(crate) fn sent(&mut self) {
        self.sent_time = Some(SystemTime::now());
        self.update_state(State::Sent);
        self.set_timeout_timer(10_000);
    }

    pub(crate) fn respond(&mut self, rsp: Rc<Message>) {
        self.rsp_time = Some(SystemTime::now());
        // rsp.set_associated_call(self.weak.clone());

        self.cancel_timeout_timer();
        self.rsp = Some(rsp.clone());

        match rsp.kind() {
            Kind::Request  => error!("Should not be request message!!"),
            Kind::Response => self.update_state(State::Responded),
            Kind::Error    => self.update_state(State::Err)
        };
    }


    // Handles a response received from an inconsistent socket
    // (e.g., due to port-mangling NAT).
	// Transitions to STALLED state to allow retry without treating as an error.
    pub(crate) fn respond_inconsistent_socket(&mut self) {
        if self.state != State::Sent {
            return;
        }
        self.update_state(State::Stalled);
    }

    // Handles a response with an incorrect method, treating it as a protocol error.
    pub(crate) fn respond_wrong_method(&mut self) {
        self.rsp_time = Some(SystemTime::now());
        self.cancel_timeout_timer();
        self.update_state(State::Err);
    }

    // Fails the RPC call with the specified cause.
    pub(crate) fn fail(&mut self) {
        if self.state.is_final() {
            return;
        }

        self.cancel_timeout_timer();
        self.update_state(State::Err);
    }
}
