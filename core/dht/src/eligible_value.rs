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

use boson_api::{Id, Value};

pub(crate) struct EligibleValue {
    target: Id,
    expected_seq: i32,
    value: Option<Value>,
    latest: bool,
}

impl EligibleValue {
    pub(crate) fn new(target: Id, expected_seq: i32) -> Self {
        Self {
            target,
            expected_seq,
            value: None,
            latest: false,
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    pub(crate) fn update(&mut self, value: Value, latest: bool) -> bool {
        if value.id() != self.target
            || (self.expected_seq >= 0 && value.sequence_number() < self.expected_seq)
            || !value.is_valid()
        {
            return false;
        }

        if self
            .value
            .as_ref()
            .map_or(true, |v| value.sequence_number() > v.sequence_number())
        {
            self.value = Some(value);
            self.latest = latest;
        }
        true
    }

    pub(crate) fn is_latest(&self) -> bool {
        self.latest
    }

    pub(crate) fn value(&self) -> Option<Value> {
        self.value.as_ref().map(|v| v.clone())
    }
}
