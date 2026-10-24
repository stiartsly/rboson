// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use super::rpccall::{RpcCall, State};

#[derive(Default)]
pub(crate) struct Listener {
    state_change_fn: Option<Box<dyn Fn(&RpcCall, State, State)>>,
    response_fn: Option<Box<dyn Fn(&RpcCall)>>,
    stall_fn: Option<Box<dyn Fn(&RpcCall)>>,
    timeout_fn: Option<Box<dyn Fn(&RpcCall)>>,
}

impl Listener {
    pub(crate) fn new<F>(f: F) -> Self
    where
        F: Fn(&RpcCall, State, State) + 'static,
    {
        Self {
            state_change_fn: Some(Box::new(f)),
            response_fn: None,
            stall_fn: None,
            timeout_fn: None,
        }
    }

    #[allow(unused)]
    pub(crate) fn response_fn<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&RpcCall) + 'static,
    {
        self.response_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn stall_fn<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&RpcCall) + 'static,
    {
        self.stall_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn timeout_fn<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&RpcCall) + 'static,
    {
        self.timeout_fn = Some(Box::new(f));
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
