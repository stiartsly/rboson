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

use boson_api::{Network, NodeInfo};

#[derive(Clone)]
pub(crate) struct Data {
    pub(crate) nodes4: Option<Vec<NodeInfo>>,
    pub(crate) nodes6: Option<Vec<NodeInfo>>,
    pub(crate) token: i32,
}

impl Data {
    pub(crate) fn new(
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>,
        token: i32,
    ) -> Self {
        Self {
            nodes4,
            nodes6,
            token,
        }
    }
}

pub(crate) trait LookupResponse {
    fn data(&self) -> &Data;

    fn nodes4(&self) -> Option<&[NodeInfo]> {
        self.data().nodes4.as_deref()
    }

    fn nodes6(&self) -> Option<&[NodeInfo]> {
        self.data().nodes6.as_deref()
    }

    fn nodes(&self, network: Network) -> Option<&[NodeInfo]> {
        match network {
            Network::IPv4 => self.nodes4(),
            Network::IPv6 => self.nodes6(),
        }
    }

    fn token(&self) -> i32 {
        self.data().token
    }
}
