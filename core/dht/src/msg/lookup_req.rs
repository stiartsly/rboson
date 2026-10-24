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

use boson_api::Id;

pub(crate) const WANT4_MASK: i32 = 0x01;
pub(crate) const WANT6_MASK: i32 = 0x02;
pub(crate) const WANT_TOKEN_MASK: i32 = 0x04;

#[derive(Clone)]
pub(crate) struct Data {
    target: Id,
    want4: bool,
    want6: bool,
    want_token: bool,
}

impl Data {
    pub(crate) fn new(target: Id, want4: bool, want6: bool, want_token: bool) -> Self {
        Self {
            target,
            want4,
            want6,
            want_token,
        }
    }
}

pub(crate) trait LookupRequest {
    fn data(&self) -> &Data;

    fn target(&self) -> &Id {
        &self.data().target
    }

    fn want4(&self) -> bool {
        self.data().want4
    }

    fn want6(&self) -> bool {
        self.data().want6
    }

    fn want_token(&self) -> bool {
        self.data().want_token
    }

    fn want(&self) -> i32 {
        (if self.want4() { 0x01 } else { 0x00 })
            | (if self.want6() { 0x02 } else { 0x00 })
            | (if self.want_token() { 0x04 } else { 0x00 })
    }
}
