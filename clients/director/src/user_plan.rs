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

use super::{Plan, Subscription};
use std::fmt;

#[derive(Clone, Debug)]
pub struct UserPlan {
    pub name: String,
    pub plan: Option<Plan>,
    pub subscription: Option<Subscription>,
}

#[allow(dead_code)]
impl UserPlan {
    pub fn new(name: String, plan: Option<Plan>, subscription: Option<Subscription>) -> Self {
        Self {
            name,
            plan,
            subscription,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }

    pub fn subscription(&self) -> Option<&Subscription> {
        self.subscription.as_ref()
    }
}

impl fmt::Display for UserPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UserPlan {{ name: {}, plan: {:?}, subscription: {:?} }}",
            self.name, self.plan, self.subscription
        )
    }
}
