use std::{
    rc::Rc,
    time::SystemTime
};
use log::error;
use crate::{
    Id,
    errors::{ProtocolError},
};
use crate::dht::{
    msg::{Body, Message, msg::Kind},
    rpc::{
        Target,
        rpc_server::RpcServer,
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
    target      : Target,

     // a tricky field to store req message before becoming Arc<Message> object.
    transient   : Option<Message>,

    req         : Option<Rc<Message>>,
    rsp         : Option<Rc<Message>>,

    sent_time   : Option<SystemTime>,
    resp_time   : Option<SystemTime>,

    state       : State,

    listener    : Option<CallListener>,

    cause: Option<Box<dyn std::error::Error + Send>>,

    _timeout_timer: Option<u64>
}

impl RpcCall {
    pub(crate) fn new(target: impl Into<Target>, mut req: Message) -> Self {
        let target: Target = target.into();
        req.set_remote(target.id(),target.socket_addr());

        Self {
            // target_reachable,
            target,
            transient       : Some(req),
            req             : None,
            rsp             : None,
            sent_time       : None,
            resp_time       : None,
            state           : State::Unsent,
            listener        : None,
            cause           : None,
            _timeout_timer   : None,
        }
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
        if let Some(req) = self.req.as_ref() {
            return req.txid();
        } else if let Some(msg) = self.transient.as_ref() {
            return msg.txid();
        } else {
            -1
        }
    }

    pub(crate) fn req(&self) -> Rc<Message> {
        self.req.as_ref().expect("Request not set").clone()
    }
    pub(crate) fn rsp(&self) -> Option<Rc<Message>> {
        self.rsp.as_ref().cloned()
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> State {
        self.state
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

    #[allow(unused)]
    pub(crate) fn resp_time(&self) -> Option<SystemTime> {
        self.resp_time
    }

    pub(crate) fn set_listener(&mut self, listener: CallListener) {
        if self.state != State::Unsent {
            return;
        }
        self.listener = Some(listener);
    }

    /*
    pub(crate) fn set_simple_listener<F>(
        &mut self,
        state_changed_cb: F
    ) -> &mut Self
    where F: Fn(&RpcCall, State, State) +'static {
        self.set_listener(CallListener::new(state_changed_cb));
        self
    }*/

    pub(crate) fn update_state(&mut self, new_state: State) {
        let prev = self.state;
        self.state = new_state;

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

    fn set_timeout_timer(&mut self, _timeout: u64) {
        // TODO: self.timeout_task = None;
        // TODO:
    }

    fn cancel_timeout_timer(&mut self) {
        // TODO: self.timeout_task = None;
    }

    fn check_timeout(&mut self) {
        self._timeout_timer = None;

        if self.state != State::Sent && self.state != State::Stalled {
            return;
        }

        let sent_time = self.sent_time.unwrap_or(SystemTime::UNIX_EPOCH);
        let elapsed = crate::elapsed_ms!(&sent_time) as u64;
        if RpcServer::RPC_CALL_TIMEOUT_MAX <= elapsed {
            self.update_state(State::Timeout);
            return;
        }

        let remaining = RpcServer::RPC_CALL_TIMEOUT_MAX.saturating_sub(elapsed);
        self.update_state(State::Stalled);
        self.set_timeout_timer(remaining);
    }

    pub(crate) fn sent(&mut self) {
        self.sent_time = Some(SystemTime::now());
        self.update_state(State::Sent);
        self.set_timeout_timer(10_000);
    }

    pub(crate) fn respond(&mut self, rsp: Rc<Message>) {
        self.resp_time = Some(SystemTime::now());
        // rsp.set_associated_call(self.weak.clone());

        self.cancel_timeout_timer();
        self.rsp = Some(rsp.clone());

        if rsp.is_err() {
            self.cause = match rsp.body() {
                Some(Body::Error(err)) => Some(ProtocolError::new(err.to_string())),
                _ => None,
            };
        }

        match rsp.kind() {
            Kind::Request  => error!("Error: should not be request message!!"),
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
        self.resp_time = Some(SystemTime::now());
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
