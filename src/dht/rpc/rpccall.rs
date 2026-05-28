use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, Duration},
    error::Error as StdError,
};
use log::error;

use crate::{
    Id,
    NodeInfo,
    errors::ProtocolError,
};
use crate::dht::{
    timer::TaskHandle,
    msg::msg::{Body, Message, Kind},
    task::candidate_node::CandidateNode,
    routing::kbucket_entry::KBucketEntry,
    rpc::{
        rpc_server::RpcServer,
        rpc_target::{Target, Reachability},
        listener::Listener as CallListener,
    },
};

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
#[repr(u32)]
pub(crate) enum State {
    Unsent,     // Call has not been sent yet
    Sent,       // Call has been sent, awaiting response
    Stalled,    // Call is delayed, possibly due to network issues
    Timeout,    // Call timed out without a response
    Canceled,   // Call was canceled before completion
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
    target_reachable: bool,

    req         : Message,
    rsp         : Option<Message>,

    sent_time   : SystemTime,
    resp_time   : SystemTime,
    expected_rtt: u64,

    state       : State,

    listener    : Option<CallListener>,

    cause: Option<Box<dyn std::error::Error + Send>>,

    timeout_task: Option<TaskHandle>,
    cloned      : Option<Arc<Mutex<RpcCall>>>,
}

impl RpcCall {
    pub(crate) fn new(
        target: Target,
        mut req: Message,
    ) -> Self
    {
        req.set_remote(target.id(), target.socket_addr());

        let target_reachable = target.is_reachable();

        Self {
            target,
            target_reachable,
            req,
            rsp: None,
            sent_time: SystemTime::UNIX_EPOCH,
            resp_time: SystemTime::UNIX_EPOCH,
            state: State::Unsent,
            listener: None,
            cause: None,
            timeout_task: None,
            expected_rtt: 0,
            cloned: None,
        }
    }

    pub(crate) fn with_node(target: NodeInfo, msg: Message) -> Self {
        Self::new(
            Target::NodeInfo(target),
            msg
        )
    }

    pub(crate) fn with_kentry(
        target: KBucketEntry,
        msg: Message
    ) -> Self {
        Self::new(
            Target::KBucketEntry(target),
            msg
        )
    }

    pub(crate) fn with_candidate(
        target: Arc<Mutex<CandidateNode>>,
        msg: Message
    ) -> Self {
        Self::new(
            Target::Candidate(target),
            msg,
        )
    }

    pub(crate) fn txid(&self) -> i32 {
        self.req.txid()
    }

    pub(crate) fn set_localid(&mut self, id: Id) {
        self.req.set_id(id);
    }

    pub(crate) fn set_cloned(&mut self, cloned: Arc<Mutex<RpcCall>>) {
        self.cloned = Some(cloned);
    }

    pub(crate) fn is_reachable_at_creation(&self) -> bool {
        self.target_reachable
    }

    pub(crate) fn target_id(&self) -> Id {
        self.target.id()
    }

    pub(crate) fn target(&self) -> &Target {
        &self.target
    }

    pub(crate) fn set_expected_rtt(&mut self, expected_rtt: u64) -> &mut Self {
        self.expected_rtt = expected_rtt;
        self
    }

    pub(crate) fn set_expected_rtt_if_absent(&mut self, expected_rtt: u64) -> &mut Self {
        if self.expected_rtt == 0 {
            self.expected_rtt = expected_rtt;
        }
        self
    }

    pub(crate) fn is_expected_rtt_set(&self) -> bool {
        self.expected_rtt > 0
    }

    pub(crate) fn expected_rtt(&self) -> u64 {
        self.expected_rtt
    }

    fn cloned1(&self) -> Arc<Mutex<RpcCall>> {
        self.cloned.as_ref()
            .map(|v| v.clone())
            .expect("panic: self cloned not set, this should never happen")
    }

    pub(crate) fn req(&self) -> &Message {
        &self.req
    }

    pub(crate) fn req_mut(&mut self) -> &mut Message {
        &mut self.req
    }

    pub(crate) fn rsp(&self) -> Option<&Message> {
        self.rsp.as_ref()
    }

    pub(crate) fn state(&self) -> State {
        self.state
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.state < State::Timeout
    }

    pub(crate) fn id_mismatched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.id() != &self.target_id()
        }).unwrap_or(true)
    }

    pub(crate) fn addr_mismatched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.remote_addr() != self.req.remote_addr()
        }).unwrap_or(true)
    }

    pub(crate) fn sent_time(&self) -> SystemTime {
        self.sent_time
    }

    pub(crate) fn resp_time(&self) -> SystemTime {
        self.resp_time
    }

    pub(crate) fn rtt(&self) -> Option<Duration> {
        self.resp_time.duration_since(self.sent_time()).ok()
    }

    pub(crate) fn cause(&self) -> Option<&(dyn StdError + Send)> {
        self.cause.as_deref()
    }

    pub(crate) fn set_listener(&mut self, listener: CallListener) {
        if self.state != State::Unsent {
            return;
        }
        self.listener = Some(listener);
    }

    pub(crate) fn set_state_changed_cb<F>(&mut self, f: F) -> &mut Self
    where F: Fn(&RpcCall, State, State) + Send + 'static {
        self.set_listener(CallListener::new(f));
        self
    }

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
        self.timeout_task = None;
        // TODO:
    }

    fn cancel_timeout_timer(&mut self) {
        self.timeout_task = None;
    }

    fn check_timeout(&mut self) {
        self.timeout_task = None;

        if self.state != State::Sent && self.state != State::Stalled {
            return;
        }

        let elapsed = crate::as_ms!(self.sent_time) as u64;
        let remaining = RpcServer::RPC_CALL_TIMEOUT_MAX.saturating_sub(elapsed);
        if remaining > 0 {
            self.update_state(State::Stalled);
            self.set_timeout_timer(remaining);
        } else {
            self.update_state(State::Timeout);
        }
    }

    pub(crate) fn sent(&mut self) {
        if self.expected_rtt <= 0 {
            return;
        }

        self.sent_time = SystemTime::now();
        self.update_state(State::Sent);
        self.set_timeout_timer(self.expected_rtt);
    }

    pub(crate) fn respond(&mut self, mut rsp: Message) {
        self.resp_time = SystemTime::now();
        rsp.set_associated_call(
            self.cloned.as_ref().unwrap().clone()
        );

        self.cancel_timeout_timer();
        self.rsp = Some(rsp);

        let rsp = self.rsp.as_ref().unwrap();
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


    // Handles a response received from an inconsistent socket (e.g., due to port-mangling NAT).
	// Transitions to STALLED state to allow retry without treating as an error.
    pub(crate) fn respond_inconsistent_socket(&mut self, _: Message) {
        if self.state != State::Sent {
            return;
        }
        self.update_state(State::Stalled);
    }

    // Handles a response with an incorrect method, treating it as a protocol error.
    pub(crate) fn respond_wrong_method(&mut self, rsp: Message) {
        self.rsp = Some(rsp);
        self.cause = Some(ProtocolError::new(format!("Got response with wrong method")));
        self.update_state(State::Err);
    }

    pub(crate) fn fail(&mut self, err: Box<dyn StdError + Send>) {
        if self.state.is_final() {
            return;
        }

        self.cancel_timeout_timer();
        self.cause = Some(err);
        self.update_state(State::Err);
    }

    #[allow(unused)]
    pub(crate) fn cancel(&mut self) {
        if self.state.is_final() {
            return;
        }

        self.cancel_timeout_timer();
        self.update_state(State::Canceled);
    }
}
