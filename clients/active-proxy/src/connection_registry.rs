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

use std::{collections::HashMap, marker::PhantomData};

pub(crate) struct ConnectionRegistry<T> {
    connections: HashMap<i32, bool>,
    inflight: usize,
    phantom: PhantomData<T>,
}

impl<T> ConnectionRegistry<T> {
    pub(crate) fn new() -> Self {
        Self {
            connections: HashMap::new(),
            inflight: 0,
            phantom: PhantomData,
        }
    }

    // Registers a newly established connection in the idle state.
    pub(crate) fn add(&mut self, connection: i32) {
        self.connections.insert(connection, false);
    }

    // Returns true if the removed connection was busy.
    pub(crate) fn remove(&mut self, connection: i32) -> bool {
        if self.connections.remove(&connection) == Some(true) {
            self.inflight -= 1;
            return true;
        }
        false
    }

    pub(crate) fn size(&self) -> usize {
        self.connections.len()
    }

    pub(crate) fn in_flight(&self) -> usize {
        self.inflight
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    // Returns true only on an idle -> busy transition.
    pub(crate) fn mark_busy(&mut self, connection: i32) -> bool {
        if let Some(state) = self.connections.get_mut(&connection) {
            if !*state {
                *state = true;
                self.inflight += 1;
                return true;
            }
        }
        false
    }

    // Returns true only on a busy -> idle transition.
    pub(crate) fn mark_idle(&mut self, connection: i32) -> bool {
        if let Some(state) = self.connections.get_mut(&connection) {
            if *state {
                *state = false;
                self.inflight -= 1;
                return true;
            }
        }
        false
    }

    pub(crate) fn connections(&self) -> Vec<i32> {
        self.connections.keys().copied().collect()
    }

    pub(crate) fn clear(&mut self) {
        self.connections.clear();
        self.inflight = 0;
    }
}
