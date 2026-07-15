use crate::Id;

/// Receives events related to friend requests.
pub trait FriendRequestListener: Send + Sync {
    /// Called when a new friend request is received.
    fn on_friend_request(&self, _user_id: &Id, _hello: Option<&str>) {}

    /// Called when a previously sent friend request was accepted.
    fn on_friend_request_accepted(&self, _user_id: &Id) {}
}
