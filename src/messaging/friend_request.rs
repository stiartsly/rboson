use std::time::SystemTime;
use crate::Id;

/// A pending or resolved friend request.
///
#[derive(Debug, Clone)]
pub struct FriendRequest {
    user_id: Id,
    initiator_id: Id,
    hello: Option<String>,
    accepted: bool,
    expired: bool,
    created_at: SystemTime,
    accepted_at: Option<SystemTime>,
    updated_at: SystemTime
}

impl FriendRequest {
    pub(crate) fn new(
        user_id: Id,
        initiator_id: Id,
        hello: Option<String>,
        created_at: SystemTime
    ) -> Self {
        Self {
            user_id,
            initiator_id,
            hello,
            accepted: false,
            expired: false,
            created_at,
            accepted_at: None,
            updated_at: created_at,
        }
    }

    /// The Id of a user who owns this request record.
    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    /// The Id of the user who initiated the request.
    pub fn initiator_id(&self) -> &Id {
        &self.initiator_id
    }

    /// The greeting / hello message attached to the request.
    pub fn hello(&self) -> Option<&str> {
        self.hello.as_deref()
    }

    /// Whether the request has been accepted.
    pub fn is_accepted(&self) -> bool {
        self.accepted
    }

    /// Whether the request has expired without being acted upon.
    pub fn is_expired(&self) -> bool {
        self.expired
    }

    pub(crate) fn accept(&mut self, accepted_at: SystemTime) {
        self.accepted = true;
        self.accepted_at = Some(accepted_at);
        self.updated_at = SystemTime::now();
    }

    /// When this request was first created.
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// When this request was accepted (`None` if not yet accepted).
    pub fn accepted_at(&self) -> Option<SystemTime> {
        self.accepted_at
    }

    /// When this record was last modified.
    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }
}
