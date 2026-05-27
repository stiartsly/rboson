use super::{
    rpccall::{RpcCall, State},
};

#[derive(Default)]
pub(crate) struct Listener {
    state_change_fn: Option<Box<dyn Fn(&RpcCall, State, State) + Send>>,
    response_fn:     Option<Box<dyn Fn(&RpcCall) + Send>>,
    stall_fn:        Option<Box<dyn Fn(&RpcCall) + Send>>,
    timeout_fn:      Option<Box<dyn Fn(&RpcCall) + Send>>,
}

impl Listener {
    pub(crate) fn new<F>(f: F) -> Self
    where F: Fn(&RpcCall, State, State) + Send + 'static {
        Self {
            state_change_fn: Some(Box::new(f)),
            response_fn: None,
            stall_fn: None,
            timeout_fn: None,
        }
    }

    #[allow(unused)]
    pub(crate) fn response_fn(&mut self, f: Box<dyn Fn(&RpcCall) + Send>) -> &mut Self {
        self.response_fn  = Some(f);
        self
    }

    #[allow(unused)]
    pub(crate) fn stall_fn(&mut self, f: Box<dyn Fn(&RpcCall) + Send>) -> &mut Self {
        self.stall_fn = Some(f);
        self
    }

    #[allow(unused)]
    pub(crate) fn timeout_fn(&mut self, f: Box<dyn Fn(&RpcCall) + Send>) -> &mut Self {
        self.timeout_fn = Some(f);
        self
    }

    pub(crate) fn on_state_change(&self, rpc_call: &RpcCall, old_state: State, new_state: State) {
        if let Some(f) = &self.state_change_fn {
            f(rpc_call, old_state, new_state);
        }
    }

    pub(crate) fn on_response(&self, rpc_call: &RpcCall) {
        if let Some(f) = &self.response_fn {
            f(rpc_call);
        }
    }

    pub(crate) fn on_stall(&self, rpc_call: &RpcCall) {
        if let Some(f) = &self.stall_fn {
            f(rpc_call);
        }
    }

    pub(crate) fn on_timeout(&self, rpc_call: &RpcCall) {
        if let Some(f) = &self.timeout_fn {
            f(rpc_call);
        }
    }
}
