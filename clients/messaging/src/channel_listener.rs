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

use boson_api::Id;
use crate::{
    Channel,
    ChannelMember
};

pub trait ChannelListener: Send + Sync {
    /// Called when a new channel was created by the local user.
    fn on_channel_created(&self, _channel: &dyn Channel) {}

    /// Called when a channel was deleted by its owner.
    fn on_channel_deleted(&self, _channel: &dyn Channel) {}

    /// Called when the local user joined a channel.
    fn on_joined_channel(&self, _channel: &dyn Channel) {}

    /// Called when the local user left a channel.
    fn on_left_channel(&self, _channel: &dyn Channel) {}

    /// Called when channel ownership was transferred.
    fn on_channel_ownership_transferred(
        &self,
        _channel: &dyn Channel,
        _old_owner: &Id,
        _new_owner: &Id,
    ) {}

    /// Called when the channel session key was rotated.
    fn on_channel_session_key_rotated(&self, _channel: &dyn Channel) {}

    /// Called when channel metadata (name, notice, etc.) was updated.
    fn on_channel_updated(&self, _channel: &dyn Channel) {}

    /// Called when a new member joined the channel.
    fn on_channel_member_joined(
        &self,
        _channel: &dyn Channel,
        _member: &ChannelMember
    ) {}

    /// Called when a member left the channel.
    fn on_channel_member_left(
        &self,
        _channel: &dyn Channel,
        _member: &ChannelMember
    ) {}

    /// Called when members were removed by an administrator.
    fn on_channel_members_removed(
        &self,
        _channel: &dyn Channel,
        _members: &[ChannelMember],
    ) {}

    /// Called when members were banned.
    fn on_channel_members_banned(
        &self,
        _channel: &dyn Channel,
        _banned: &[ChannelMember],
    ) {}

    /// Called when members were unbanned.
    fn on_channel_members_unbanned(
        &self,
        _channel: &dyn Channel,
        _unbanned: &[ChannelMember],
    ) {}

    /// Called when member roles were updated.
    fn on_channel_members_role_updated(
        &self,
        _channel: &dyn Channel,
        _members: &[ChannelMember],
    ) {}
}
