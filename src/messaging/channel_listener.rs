use crate::Id;
use crate::messaging::channel::{Channel, ChannelMember};

/// Receives channel lifecycle and membership events.
pub trait ChannelListener: Send + Sync {
    /// Called when a new channel was created by the local user.
    fn on_channel_created(&self, channel: &dyn Channel) {}

    /// Called when a channel was deleted by its owner.
    fn on_channel_deleted(&self, channel: &dyn Channel) {}

    /// Called when the local user joined a channel.
    fn on_joined_channel(&self, channel: &dyn Channel) {}

    /// Called when the local user left a channel.
    fn on_left_channel(&self, channel: &dyn Channel) {}

    /// Called when channel ownership was transferred.
    fn on_channel_ownership_transferred(
        &self,
        channel:   &dyn Channel,
        old_owner: &Id,
        new_owner: &Id,
    ) {}

    /// Called when the channel session key was rotated.
    fn on_channel_session_key_rotated(&self, channel: &dyn Channel) {}

    /// Called when channel metadata (name, notice, etc.) was updated.
    fn on_channel_updated(&self, channel: &dyn Channel) {}

    /// Called when a new member joined the channel.
    fn on_channel_member_joined(&self, channel: &dyn Channel, member: &dyn ChannelMember) {}

    /// Called when a member left the channel.
    fn on_channel_member_left(&self, channel: &dyn Channel, member: &dyn ChannelMember) {}

    /// Called when members were removed by an administrator.
    fn on_channel_members_removed(&self, channel: &dyn Channel, members: &[Box<dyn ChannelMember>]) {}

    /// Called when members were banned.
    fn on_channel_members_banned(&self, channel: &dyn Channel, banned: &[Box<dyn ChannelMember>]) {}

    /// Called when members were unbanned.
    fn on_channel_members_unbanned(&self, channel: &dyn Channel, unbanned: &[Box<dyn ChannelMember>]) {}

    /// Called when member roles were updated.
    fn on_channel_members_role_updated(&self, channel: &dyn Channel, members: &[Box<dyn ChannelMember>]) {}
}
