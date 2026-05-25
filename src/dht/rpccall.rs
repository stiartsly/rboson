use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::time::Duration;

use crate::{
    Id,
    NodeInfo,
    errors::ProtocolError,
};
use super::{
    node_entry::NodeEntry,
    node_entry::Reachability,
    timer::TaskHandle,
    msg::msg::{self, Body, Message},
    task::candidate_node::CandidateNode,
    routing::kbucket_entry::KBucketEntry,
    server::RpcServer,
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
    target: NodeEntry,
    target_reachable: bool,

    req: Arc<Mutex<Message>>,
    rsp: Option<Arc<Mutex<Message>>>,

    sent_time   : Option<SystemTime>,
    resp_time   : Option<SystemTime>,
    expected_rtt: u64,

    state       : State,

    state_changed_fn: Box<dyn for<'a> Fn(&'a RpcCall, State, State) + Send>,
    responsed_fn: Box<dyn for<'a> Fn(&'a RpcCall, Arc<Mutex<Message>>) + Send>,
    stalled_fn: Box<dyn for<'a> Fn(&'a RpcCall) + Send>,
    timeout_fn: Box<dyn for<'a> Fn(&'a RpcCall) + Send>,

    cause: Option<Box<dyn std::error::Error + Send>>,

    timeout_task: Option<TaskHandle>,
    cloned: Option<Arc<Mutex<RpcCall>>>,


}

impl RpcCall {
    fn new(target: NodeEntry, req: Arc<Mutex<Message>>) -> Self
    {
        req.lock().unwrap().set_remote(
            target.id(),
            target.socket_addr()
        );

        let target_reachable = target.is_reachable();

        Self {
            target,
            target_reachable,
            req,
            rsp: None,
            sent_time: None,
            resp_time: None,
            state: State::Unsent,
            state_changed_fn: Box::new(|_, _, _| {}),
            responsed_fn: Box::new(|_, _| {}),
            stalled_fn: Box::new(|_| {}),
            timeout_fn: Box::new(|_| {}),
            cause: None,
            timeout_task: None,
            cloned: None,
            expected_rtt: 0,
        }
    }

    pub(crate) fn with_node(
        target: NodeInfo,
        msg: Arc<Mutex<Message>>
    ) -> Self {
        Self::new(
            NodeEntry::NodeInfo(target),
            msg
        )
    }

    pub(crate) fn with_kentry(
        target: KBucketEntry,
        msg: Arc<Mutex<Message>>
    ) -> Self {
        Self::new(
            NodeEntry::KBucketEntry(target),
            msg
        )
    }

    pub(crate) fn with_candidate(
        target: Arc<Mutex<CandidateNode>>,
        msg: Arc<Mutex<Message>>
    ) -> Self {
        Self::new(
            NodeEntry::CandidateNode(target),
            msg
        )
    }

    pub(crate) fn txid(&self) -> i32 {
        self.req().lock().unwrap().txid()
    }

    pub(crate) fn set_localid(&mut self, id: Id) -> &mut Self {
        self.req.lock().unwrap().set_id(id);
        self
    }

    pub(crate) fn is_reachable_at_creation_time(&self) -> bool {
        self.target_reachable
    }

    pub(crate) fn target_id(&self) -> Id {
        self.target.id()
    }

    pub(crate) fn target(&self) -> &NodeEntry {
        &self.target
    }

    pub(crate) fn target_mut(&mut self) -> &mut NodeEntry {
        &mut self.target
    }

    pub(crate) fn set_cloned(&mut self, cloned: Arc<Mutex<RpcCall>>) {
        self.cloned = Some(cloned);
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

    fn cloned(&self) -> Arc<Mutex<RpcCall>> {
        self.cloned.as_ref()
            .map(|v| v.clone())
            .expect("panic: self cloned not set, this should never happen")
    }

    pub(crate) fn req(&self) -> Arc<Mutex<Message>> {
        self.req.clone()
    }

    pub(crate) fn rsp(&self) -> Option<Arc<Mutex<Message>>> {
        self.rsp.as_ref().cloned()
    }

    pub(crate) fn state(&self) -> State {
        self.state
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.state < State::Timeout
    }

    pub(crate) fn id_matched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.lock().unwrap().id() == &self.target_id()
        }).unwrap_or(false)
    }

    pub(crate) fn id_mismatched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.lock().unwrap().id() != &self.target_id()
        }).unwrap_or(true)
    }

    pub(crate) fn addr_mismatched(&self) -> bool {
        self.rsp.as_ref().map(|v| {
            v.lock().unwrap().remote_addr() != self.req().lock().unwrap().remote_addr()
        }).unwrap_or(true)
    }

    pub(crate) fn sent_time(&self) -> Option<&SystemTime> {
        self.sent_time.as_ref()
    }

    pub(crate) fn resp_time(&self) -> Option<&SystemTime> {
        self.resp_time.as_ref()
    }

    pub(crate) fn rtt(&self) -> Option<Duration> {
        let Some(ref sent_time) = self.sent_time else {
            return None;
        };
        let Some(ref resp_time) = self.resp_time else {
            return None;
        };

        resp_time.duration_since(*sent_time).ok()
    }

    pub(crate) fn cause(&self) -> Option<&(dyn std::error::Error + Send)> {
        self.cause.as_deref()
    }

    pub(crate) fn set_state_changed_cb<F>(&mut self, f: F)
    where F: for<'a> Fn(&'a RpcCall, State, State) + Send + 'static {
        self.state_changed_fn = Box::new(f);
    }

    pub(crate) fn set_responsed_cb(
        &mut self,
        f: Box<dyn for<'a> Fn(&'a RpcCall, Arc<Mutex<Message>>) + Send>,
    ) {
        self.responsed_fn = f;
    }

    pub(crate) fn set_stalled_cb(
        &mut self,
        f: Box<dyn for<'a> Fn(&'a RpcCall) + Send>,
    ) {
        self.stalled_fn = f;
    }

    pub(crate) fn set_timeout_cb(
        &mut self,
        f: Box<dyn for<'a> Fn(&'a RpcCall) + Send>,
    ) {
        self.timeout_fn = f;
    }

    pub(crate) fn update_state(&mut self, new_state: State) {
        let prev = self.state;
        self.state = new_state;

        (self.state_changed_fn)(self, prev, new_state);
        match new_state {
            State::Responded => {
                if let Some(rsp) = self.rsp() {
                    (self.responsed_fn)(self, rsp);
                }
            }
            State::Stalled => (self.stalled_fn)(self),
            State::Timeout => (self.timeout_fn)(self),
            _ => {}
        }
    }

    fn set_timeout(&mut self, timeout: u64) {
        self.timeout_task = None;

        /* let Some(scheduler) = self.scheduler.as_ref().cloned() else {
            return;
        };

        let call = self.cloned();
        self.timeout_task = scheduler.lock().unwrap().add(
            Duration::from_millis(timeout),
            None,
            move || {
                let call = call.clone();
                Box::pin(async move {
                    call.lock().unwrap().check_timeout();
                })
            },
        ).ok();
        */
    }

    fn cancel_timeout(&mut self) {
        self.timeout_task = None;
    }

    fn check_timeout(&mut self) {
        self.timeout_task = None;

        if self.state != State::Sent && self.state != State::Stalled {
            return;
        }

        let Some(sent_time) = self.sent_time else {
            return;
        };

        use crate::as_ms;
        let elapsed = as_ms!(sent_time) as u64;
        let remaining = RpcServer::RPC_CALL_TIMEOUT_MAX.saturating_sub(elapsed);

        if remaining > 0 {
            self.update_state(State::Stalled);
            self.set_timeout(remaining);
        } else {
            self.update_state(State::Timeout);
        }
    }

    pub(crate) fn sent(&mut self) {
        if self.expected_rtt <= 0 {
            return;
        }

        self.sent_time = Some(SystemTime::now());
        self.update_state(State::Sent);
        self.set_timeout(self.expected_rtt);
    }

    pub(crate) fn respond(&mut self, rsp: Arc<Mutex<Message>>) {
        self.resp_time = Some(SystemTime::now());
        rsp.lock().unwrap().set_associated_call(self.cloned());

        self.cancel_timeout();
        self.rsp = Some(rsp.clone());

        if rsp.lock().unwrap().is_err() {
            self.cause = match rsp.lock().unwrap().body() {
                Some(Body::Error(err)) => Some(ProtocolError::new(err.to_string())),
                _ => Some(ProtocolError::new("Remote call failed".to_string())),
            };
        }

        match rsp.lock().unwrap().kind() {
            msg::Kind::Request  => panic!("INTERNAL ERROR: invalid response type!!"),
            msg::Kind::Response => self.update_state(State::Responded),
            msg::Kind::Error    => self.update_state(State::Err)
        }
    }


    // Handles a response received from an inconsistent socket (e.g., due to port-mangling NAT).
	// Transitions to STALLED state to allow retry without treating as an error.
    pub(crate) fn respond_inconsistent_socket(&mut self, _msg: Arc<Mutex<Message>>) {
        if self.state != State::Sent {
            return;
        }
        self.update_state(State::Stalled);
    }

    // Handles a response with an incorrect method, treating it as a protocol error.
    pub(crate) fn respond_wrong_method(&mut self, msg: Arc<Mutex<Message>>) {
       // self.rsp = Some(msg.clone());
        self.cause = Some(ProtocolError::new(format!("Got response with wrong method")));
        self.update_state(State::Err);
    }

    pub(crate) fn fail(&mut self, err: Box<dyn std::error::Error + Send>) {
        if self.state.is_final() {
            return;
        }

        self.cause = Some(err);
        self.cancel_timeout();
        self.update_state(State::Err);
    }

    pub(crate) fn cancel(&mut self) {
        if self.state.is_final() {
            return;
        }

        self.cancel_timeout();
        self.update_state(State::Canceled);
    }
}
