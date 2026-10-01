use std::time::SystemTime;
use crate::messaging::friend_request::FriendRequest;
use crate::Id;

#[derive(Debug, Clone)]
pub struct PhotonFriendRequest {
    pub user_id: Id,
    pub initiator_id: Id,
    pub hello: Option<String>,
    pub accepted: bool,
    pub expired: bool,
    pub created_at: SystemTime,
    pub accepted_at: Option<SystemTime>,
    pub updated_at: SystemTime,
}

impl FriendRequest for PhotonFriendRequest {
    fn user_id(&self) -> &Id {
        &self.user_id
    }

    fn initiator_id(&self) -> &Id {
        &self.initiator_id
    }

    fn hello(&self) -> Option<&str> {
        self.hello.as_deref()
    }

    fn is_accepted(&self) -> bool {
        self.accepted
    }

    fn is_expired(&self) -> bool {
        self.expired
    }

    fn created_at(&self) -> SystemTime {
        self.created_at
    }

    fn accepted_at(&self) -> Option<SystemTime> {
        self.accepted_at
    }

    fn updated_at(&self) -> SystemTime {
        self.updated_at
    }
}
