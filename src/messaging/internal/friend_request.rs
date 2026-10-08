use std::time::SystemTime;
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
