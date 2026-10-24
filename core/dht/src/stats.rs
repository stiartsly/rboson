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

use std::fmt;
use boson_api::msg::msg;

pub(crate) struct Stats {
    // TODO;
}

impl Stats {
    pub(crate) fn new() -> Self {
        Stats {}
    }

    pub(crate) fn received_bytes(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn sent_bytes(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn received_bytes_in_sec(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn sent_bytes_in_sec(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn received_msgs(&self, _: msg::Method, _: msg::Kind) -> usize {
        unimplemented!()
    }

    pub(crate) fn total_received_msgs(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn sent_msgs(&self, _: msg::Method, _: msg::Kind) -> usize {
        unimplemented!()
    }

    pub(crate) fn total_sent_msgs(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn timeout_msgs(&self, _: msg::Method) -> usize {
        unimplemented!()
    }

    pub(crate) fn total_timeout_msgs(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn dropped_packets(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn dropped_bytes(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn on_received_bytes(&mut self, _: usize) {
        unimplemented!()
    }

    pub(crate) fn on_sent_bytes(&mut self, _: usize) {
        unimplemented!()
    }

    pub(crate) fn on_received_msg(&mut self, _: &Box<dyn msg::Msg>) {
        unimplemented!()
    }

    pub(crate) fn on_sent_msg(&mut self, _: &Box<dyn msg::Msg>) {
        unimplemented!()
    }

    pub(crate) fn on_timeout_msg(&mut self, _: &Box<dyn msg::Msg>) {
        unimplemented!()
    }

    pub(crate) fn on_dropped_packet(&mut self, _: usize) {
        unimplemented!()
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
