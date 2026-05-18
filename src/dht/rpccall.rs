use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::time::Duration;

use crate::{
    Id,
    NodeInfo,
    errors::ProtocolError,
};
use crate::dht::{
    node_entry::NodeEntry,
    dht::DHT,
    scheduler::Scheduler,
    msg::msg::{self, Message},
    task::candidate_node::CandidateNode,
    routing::kbucket_entry::KBucketEntry,
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
        self > &State::Timeout
    }
}

pub(crate) struct RpcCall {
    target: NodeEntry,

    req: Arc<Mutex<Message>>,
    rsp: Option<Arc<Mutex<Message>>>,

    sent_time: Option<SystemTime>,
    response_time: Option<SystemTime>,

    state: State,

    state_changed_fn: Box<dyn Fn(&RpcCall, State, State) + Send>,
    responsed_fn: Box<dyn Fn(&RpcCall, Arc<Mutex<Message>>) + Send>,
    stalled_fn: Box<dyn Fn(&RpcCall) + Send>,
    timeout_fn: Box<dyn Fn(&RpcCall) + Send>,

    cause: Option<Box<dyn std::error::Error + Send>>,

    dht: Arc<Mutex<DHT>>,
    scheduler: Option<Arc<Mutex<Scheduler>>>,
    cloned: Option<Arc<Mutex<RpcCall>>>,

    expected_rtt: u64,
}

impl RpcCall {
    fn new(target: NodeEntry,
        dht: Arc<Mutex<DHT>>,
        req: Arc<Mutex<Message>>) -> Self
    {
        req.lock().unwrap().set_remote(
            target.id(),
            target.socket_addr()
        );

        Self {
            target,
            req,
            rsp: None,
            sent_time: None,
            response_time: None,
            state: State::Unsent,
            state_changed_fn: Box::new(|_, _, _| {}),
            responsed_fn: Box::new(|_, _| {}),
            stalled_fn: Box::new(|_| {}),
            timeout_fn: Box::new(|_| {}),
            cause: None,
            dht,
            scheduler: None,
            cloned: None,
            expected_rtt: 0,
        }
    }

    pub(crate) fn from_node(
        target: NodeInfo,
        dht: Arc<Mutex<DHT>>,
        msg: Arc<Mutex<Message>>
    ) -> Self {
        Self::new(NodeEntry::NodeInfo(target), dht, msg)
    }

    pub(crate) fn from_kentry(
        target: KBucketEntry,
        dht: Arc<Mutex<DHT>>,
        msg: Arc<Mutex<Message>>
    ) -> Self {
        Self::new(NodeEntry::KBucketEntry(target), dht, msg)
    }

    pub(crate) fn from_candidate(
        target: Arc<Mutex<CandidateNode>>,
        dht: Arc<Mutex<DHT>>,
        msg: Arc<Mutex<Message>>
    ) -> Self {
        Self::new(NodeEntry::CandidateNode(target), dht, msg)
    }

    pub(crate) fn txid(&self) -> i32 {
        self.req().lock().unwrap().txid()
    }

    pub(crate) fn dht(&self) -> Arc<Mutex<DHT>> {
        self.dht.clone()
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

    fn cloned(&self) -> Arc<Mutex<RpcCall>> {
        self.cloned.as_ref()
            .map(|v| v.clone())
            .expect("panic: self cloned not set, this should never happen")
    }

    fn scheduler(&self) -> Arc<Mutex<Scheduler>> {
        self.scheduler.as_ref()
            .map(|v| v.clone())
            .expect("panic: scheduler not set, this should never happen")
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

    pub(crate) fn response_time(&self) -> Option<&SystemTime> {
        self.response_time.as_ref()
    }

    pub(crate) fn rtt(&self) -> Option<Duration> {
        let Some(ref sent_time) = self.sent_time else {
            return None;
        };
        let Some(ref response_time) = self.response_time else {
            return None;
        };

        response_time.duration_since(*sent_time).ok()
    }

    pub(crate) fn set_state_changed_cb<F>(&mut self, f: F)
    where F: Fn(&RpcCall, State, State) + Send + 'static {
        self.state_changed_fn = Box::new(f);
    }

    pub(crate) fn set_responsed_cb<F>(&mut self, f: Box<F>)
    where F: Fn(&RpcCall, Arc<Mutex<Message>>) + Send + 'static {
        self.responsed_fn = f;
    }

    pub(crate) fn set_stalled_cb<F>(&mut self, f: Box<F>)
    where F: Fn(&RpcCall) + Send + 'static {
        self.stalled_fn = f;
    }

    pub(crate) fn set_timeout_cb<F>(&mut self, f: Box<F>)
    where F: Fn(&RpcCall) + Send + 'static {
        self.timeout_fn = f;
    }

    pub(crate) fn update_state(&mut self, new_state: State) {
        let prev = self.state;
        self.state = new_state;

        if new_state == State::Timeout {
            // TODO:
        }

        // TODO:
    }

    fn set_timeout(&mut self, _timeout: u64) {
        unimplemented!()
    }

    fn cancel_timeout(&mut self) {
        unimplemented!()
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
        self.response_time = Some(SystemTime::now());
        rsp.lock().unwrap().set_associated_call(self.cloned());

        self.cancel_timeout();
        self.rsp = Some(rsp.clone());

        if rsp.lock().unwrap().is_err() {
            //TODO: handle error response.
            return;
        }

        match rsp.lock().unwrap().kind() {
            msg::Kind::Request  => panic!("INTERNAL ERROR: invalid response type!!"),
            msg::Kind::Response => self.update_state(State::Responded),
            msg::Kind::Error    => self.update_state(State::Err)
        }
    }


    // Handles a response received from an inconsistent socket (e.g., due to port-mangling NAT).
	// Transitions to STALLED state to allow retry without treating as an error.
    pub(crate) fn respond_inconsistent_socket(&mut self, msg: Arc<Mutex<Message>>) {
        if self.state != State::Sent {
            return;
        }
        self.update_state(State::Stalled);
    }

    // Handles a response with an incorrect method, treating it as a protocol error.
    pub(crate) fn respond_wrong_method(&mut self, msg: Arc<Mutex<Message>>) {
        self.rsp = Some(msg.clone());
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

    /*
    pub(crate) fn check_timeout(&mut self) {
        if self.state != State::Sent && self.state != State::Stalled {
            return;
        }

        let timeout = Duration::from_millis(constants::RPC_CALL_TIMEOUT_MAX);
        let elapsed = self.sent.elapsed().unwrap();

        if timeout > elapsed {
            let remaining = (timeout - elapsed).as_millis() as u64;

            self.update_state(State::Stalled);

            let cloned_call = self.cloned();
            let cloned_sche = self.scheduler();

            cloned_sche.lock().unwrap().add_oneshot(move || {
                cloned_call.lock().unwrap().check_timeout()
            }, remaining);

        } else {
            self.update_state(State::Timeout);
        }
    }
    */
}
