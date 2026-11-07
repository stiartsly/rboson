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

use std::time::SystemTime;

use boson_api::Id;

/// A pending or resolved friend request.
///
#[derive(Debug, Clone)]
pub struct FriendRequest {
    user_id     : Id,
    initiator_id: Id,
    hello       : Option<String>,
    accepted    : bool,

    created_at  : u64,
    accepted_at : Option<u64>,
    updated_at  : u64
}

impl FriendRequest {
    const EXPIRATION_SECS: u128 = 60 * 60 * 24 * 7; // 1 week in seconds

    pub(crate) fn restore(
        user_id: Id,
        initiator_id: Id,
        hello: Option<String>,
        accepted: bool,
        created_at: u64,
        accepted_at: Option<u64>,
        updated_at: u64,
    ) -> Self {
        Self {
            user_id,
            initiator_id,
            hello,
            accepted,
            created_at: created_at as u64,
            accepted_at: accepted_at.map(|v| v as u64),
            updated_at: updated_at as u64,
        }
    }

    pub(crate) fn new(
        user_id: Id,
        initiator_id: Id,
        hello: Option<String>,
        created_at: SystemTime,
    ) -> Self {
        let created_at = boson_api::as_ms!(created_at) as u64;
        Self {
            user_id, initiator_id, hello,
            accepted: false,
            created_at,
            accepted_at: None,
            updated_at: created_at,
        }
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn initiator_id(&self) -> &Id {
        &self.initiator_id
    }

    pub fn hello(&self) -> Option<&str> {
        self.hello.as_deref()
    }

    pub fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub(crate) fn accept_at(&mut self, accepted_at: SystemTime) {
        self.accepted = true;
        self.accepted_at = Some(boson_api::as_ms!(accepted_at) as u64);
        self.updated_at = boson_api::as_ms!(SystemTime::now()) as u64;
    }

    pub fn is_expired(&self) -> bool {
        self.is_expired_at(SystemTime::now())
    }

    /// Checks whether the request had expired at the given time.
    ///
    /// @param time the time to check.
    /// @return {@code true} if the request was still pending and past its expiry
    /// at {@code time}.
	pub(crate) fn is_expired_at(&self, time: SystemTime) -> bool {
        !self.accepted &&
            boson_api::elapsed_ms!(time) >= Self::EXPIRATION_SECS * 1000
    }

    pub fn created_at(&self) -> SystemTime {
        boson_api::from_ms!(self.created_at)
    }

    pub fn accepted_at(&self) -> Option<SystemTime> {
        self.accepted_at.map(|v| boson_api::from_ms!(v))
    }

    pub fn updated_at(&self) -> SystemTime {
        boson_api::from_ms!(self.updated_at)
    }
}
