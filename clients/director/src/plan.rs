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

use serde::Deserialize;
use std::fmt;

#[derive(Clone, Debug, Deserialize)]
pub enum Cycle {
    Monthly,
    Annually,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Plan {
    id: i32,
    name: String,

    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    price: f64,
    currency: String,
    cycle: Cycle,
    #[serde(default)]
    annually_discount: f64,
    #[serde(default)]
    active: bool,
}

impl Plan {
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    pub fn detail(&self) -> Option<&String> {
        self.detail.as_ref()
    }

    pub fn price(&self) -> f64 {
        self.price
    }

    pub fn currency(&self) -> &str {
        &self.currency
    }

    pub fn cycle(&self) -> &Cycle {
        &self.cycle
    }

    pub fn annually_discount(&self) -> f64 {
        self.annually_discount
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn is_free(&self) -> bool {
        self.price == 0.0
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Plan {{ id: {}, name: {}, price: {}, currency: {}, cycle: {:?} }}",
            self.id, self.name, self.price, self.currency, self.cycle
        )
    }
}
