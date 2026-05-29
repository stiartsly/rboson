use std::time::SystemTime;
use crate::Id;
use crate::messaging::contact::Contact;

/// A conversation between the local user and another party (person or channel).
///
/// The conversation ID equals the other party's boson `Id`.
pub trait Conversation: Send + Sync {
    /// The unique identifier of this conversation (same as the participant's `Id`).
    fn id(&self) -> &Id;

    /// Display title – typically the contact's `display_name()`.
    fn title(&self) -> &str;

    /// Optional avatar URI / identifier.
    fn avatar(&self) -> Option<&str>;

    /// The contact representing the other participant.
    fn contact(&self) -> &dyn Contact;

    /// Whether this is a channel (group) conversation.
    fn is_channel(&self) -> bool;

    /// Short text preview of the most recent activity.
    fn preview(&self) -> &str;

    /// When this conversation was last updated.
    fn updated_at(&self) -> Option<SystemTime>;

    /// Total number of messages in this conversation.
    fn message_count(&self) -> usize;

    /// Number of messages the local user has not yet read.
    fn unread_count(&self) -> usize;

    /// Whether this conversation is muted (no notifications).
    fn is_muted(&self) -> bool;

    /// Whether this conversation is pinned to the top.
    fn is_pinned(&self) -> bool;
}
