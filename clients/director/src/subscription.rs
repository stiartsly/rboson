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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Active,
    PastDue,
    Expired,
    Canceled,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Subscription {
    #[serde(rename = "id")]
    pub id: u64,
    #[serde(rename = "planId")]
    pub plan_id: i32,
    #[serde(rename = "planName", default)]
    pub plan_name: Option<String>,
    #[serde(rename = "status")]
    pub status: Status,
    #[serde(rename = "startDate")]
    pub start_date: u64,
    #[serde(rename = "endDate")]
    pub end_date: u64,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
}

impl Subscription {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn plan_id(&self) -> i32 {
        self.plan_id
    }

    pub fn plan_name(&self) -> Option<&str> {
        self.plan_name.as_deref()
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn start_date(&self) -> u64 {
        self.start_date
    }

    pub fn end_date(&self) -> u64 {
        self.end_date
    }

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn updated_at(&self) -> u64 {
        self.updated_at
    }
}

impl std::fmt::Display for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Subscription{{id={}, planId={}, status={}, startDate={}, endDate={}}}",
            self.id, self.plan_id, self.status, self.start_date, self.end_date
        )
    }
}
