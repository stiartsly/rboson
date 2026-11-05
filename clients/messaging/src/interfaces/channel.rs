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
    channel::Channel as ChannelStruct,
    channel::ChannelEditor as ChannelEditorStruct,
    channel::Permission,
    channel_member::ChannelMember,
    interfaces::ContactOps,
};

/// A channel contact – a group conversation on the boson network.
///
/// Extends [`ContactOps`] with channel-specific information.
pub trait Channel: ContactOps + Sync + Send {
    /// The channel's join / invite permission policy.
    fn permission(&self) -> Permission;

    /// An optional notice for the channel.
    fn notice(&self) -> Option<&str>;

    /// Whether the channel is announced to the network.
    fn is_announced(&self) -> bool;

    /// Total number of tracked members, including banned members.
    fn size(&self) -> usize;

    /// Number of banned members.
    fn banned(&self) -> usize;

    /// Members ordered by join time, with unbanned members first.
    fn members(&self) -> Vec<ChannelMember>;

    /// Look up a member by ID.
    fn member(&self, member_id: &Id) -> Option<ChannelMember>;

    fn has_member(&self, member_id: &Id) -> bool {
        self.member(member_id).is_some()
    }

    /// Creates an editor that builds a new channel without mutating this one.
    fn edit_channel(&self) -> ChannelEditorStruct;
}

/// Builder for an immutable channel update.
pub trait ChannelEditor: Send + Sync {
    /// Update the channel permission.
    fn permission(self, permission: Permission) -> Self;

    /// Update the channel's display name.
    fn name(self, name: String) -> Self;

    /// Update the channel notice.
    fn notice(self, notice: Option<String>) -> Self;

    /// Update whether the channel is announced.
    fn announced(self, announced: bool) -> Self;

    /// Build the updated immutable channel.
    fn build(self) -> ChannelStruct;
}
