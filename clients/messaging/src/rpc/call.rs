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

use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    pin::Pin,
    task::{Context,Poll}
};
use log::debug;

use boson_api::{
    errors::StateError,
    Id,
    Promise,
    PromiseFuture,
    Result,
};
use crate::errors::{
    AuthenticationError,
    EncodingError,
};
use super::{
    method::Method,
    response::{Params, RpcResponse},
};

struct PendingCall {
    method: Method,
    recipient: Id,
    promise: Promise<Params>,
}

#[derive(Default)]
pub(crate) struct Calls {
    pending: RefCell<HashMap<i64, PendingCall>>,
}

pub(crate) struct CallFuture<'a> {
    calls: &'a Calls,
    id: i64,
    future: PromiseFuture<Params>,
}

impl Future for CallFuture<'_> {
    type Output = Result<Params>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().future).poll(context)
    }
}

impl Drop for CallFuture<'_> {
    fn drop(&mut self) {
        self.calls.pending.borrow_mut().remove(&self.id);
    }
}

impl Calls {
    pub(crate) fn register(
        &self,
        id: i64,
        method: Method,
        recipient: Id,
    ) -> Result<CallFuture<'_>> {
        if id < 0 || self.pending.borrow().contains_key(&id) {
            return Err(StateError::new("Invalid or duplicate pending RPC ID"));
        }
        let (promise, future) = Promise::pair();
        self.pending.borrow_mut().insert(
            id,
            PendingCall { method, recipient, promise },
        );
        Ok(CallFuture {
            calls: self,
            id,
            future,
        })
    }

    pub(crate) fn respond(&self, response: RpcResponse, from: &Id) -> Result<()> {
        let rsp_id = response.id();
        let method = response.method();

        let call = self.pending.borrow_mut().remove(&rsp_id);
        let Some(call) = call else {
            debug!("Ignoring RPC response {} without a pending call",rsp_id);
            return Ok(());
        };

        if &call.recipient != from {
            return Err(AuthenticationError::new(
                "RPC response sender does not match request recipient",
            ));
        }

        if call.method != method {
            return Err(EncodingError::new(format!(
                "RPC response method '{:?}' does not match '{:?}'",
                method, call.method
            )).into())
        }

        call.promise.complete(response.result());
        Ok(())
    }

    pub(crate) fn clear(&self, reason: &str) {
        for (_, call) in self.pending.take() {
            call.promise.complete(
                Err(StateError::new(reason.to_string()).into())
            );
        }
    }
}
