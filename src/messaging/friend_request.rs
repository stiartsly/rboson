use std::time::SystemTime;
use crate::Id;

/// A pending or resolved friend request.
///
/// Mirrors the Java `FriendRequest` interface.
pub trait FriendRequest: Send + Sync {
    /// The boson `Id` of the local user who owns this request record.
    fn user_id(&self) -> &Id;

    /// The boson `Id` of the user who initiated the request.
    fn initiator_id(&self) -> &Id;

    /// The greeting / hello message attached to the request.
    fn hello(&self) -> Option<&str>;

    /// Whether the request has been accepted.
    fn is_accepted(&self) -> bool;

    /// Whether the request has expired without being acted upon.
    fn is_expired(&self) -> bool;

    /// When this request was first created.
    fn created_at(&self) -> SystemTime;

    /// When this request was accepted (`None` if not yet accepted).
    fn accepted_at(&self) -> Option<SystemTime>;

    /// When this record was last modified.
    fn updated_at(&self) -> SystemTime;
}
