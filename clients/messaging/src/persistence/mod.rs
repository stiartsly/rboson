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

//! Local persistence for the messaging client.
//!
//! `MessagingStore` is the storage port. It mirrors the Java
//! `MessagingStore` contract and uses plain row records so the SQLite
//! implementation can be replaced by another platform backend later.

use std::path::Path;

use boson_api::{Id, Result};

pub(crate) mod database;

#[derive(Debug, Clone)]
pub(crate) struct StoredMessage {
    pub(crate) rid: i64,
    pub(crate) id: Id,
    pub(crate) conversation_id: Id,
    pub(crate) version: i32,
    pub(crate) recipient: Id,
    pub(crate) message_type: i32,
    pub(crate) from: Option<Id>,
    pub(crate) created_at: i64,
    pub(crate) content_type: Option<String>,
    pub(crate) content_disposition: Option<String>,
    pub(crate) payload: Option<Vec<u8>>,
    pub(crate) sent_at: i64,
    pub(crate) received_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredChannel {
    pub(crate) id: Id,
    pub(crate) owner: Id,
    pub(crate) permission: i32,
    pub(crate) notice: Option<String>,
    pub(crate) announce: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredContact {
    pub(crate) id: Id,
    pub(crate) contact_type: i32,
    pub(crate) session_key: Option<Vec<u8>>,
    pub(crate) name: Option<String>,
    pub(crate) remark: Option<String>,
    pub(crate) tags: Option<String>,
    pub(crate) muted: bool,
    pub(crate) blocked: bool,
    pub(crate) revision: i32,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) channel: Option<StoredChannel>,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredChannelMember {
    pub(crate) id: Id,
    pub(crate) channel_id: Id,
    pub(crate) role: i32,
    pub(crate) joined: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredFriendRequest {
    pub(crate) id: Id,
    pub(crate) initiator: Id,
    pub(crate) hello: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) accepted: bool,
    pub(crate) accepted_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredConversation {
    pub(crate) contact: StoredContact,
    pub(crate) last_message: Option<StoredMessage>,
}

#[allow(dead_code)]
pub(crate) trait MessagingStore {
    fn open(&mut self, path: &Path) -> Result<()>;
    fn close(&mut self);
    fn is_open(&self) -> bool;

    fn get_contacts_revision(&mut self) -> Result<i32>;
    fn put_contact(&mut self, revision: i32, contact: &StoredContact) -> Result<()>;
    fn put_contacts(&mut self, revision: i32, contacts: &[StoredContact]) -> Result<()>;
    fn put_contact_locally(&mut self, contact: &StoredContact) -> Result<()>;
    fn remove_contact_locally(&mut self, contact_id: &Id) -> Result<bool>;
    fn remove_contact(&mut self, revision: i32, contact_id: &Id) -> Result<bool>;
    fn remove_contacts(&mut self, revision: i32, contact_ids: &[Id]) -> Result<bool>;
    fn clear_contacts(&mut self, revision: i32) -> Result<()>;
    fn get_contact(&mut self, contact_id: &Id) -> Result<Option<StoredContact>>;
    fn get_contacts(&mut self, contact_ids: &[Id]) -> Result<Vec<StoredContact>>;
    fn get_all_contacts(&mut self) -> Result<Vec<StoredContact>>;
    fn exists_contact(&mut self, contact_id: &Id) -> Result<bool>;

    fn put_message(&mut self, message: &StoredMessage) -> Result<i64>;
    fn update_message_sent_time(&mut self, message_id: &Id, sent_at: i64) -> Result<()>;
    fn get_messages_in_range(
        &mut self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<StoredMessage>>;
    fn get_messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<StoredMessage>>;
    fn remove_message(&mut self, rid: i64) -> Result<bool> {
        self.remove_messages(&[rid])
    }
    fn remove_messages(&mut self, rids: &[i64]) -> Result<bool>;
    fn remove_messages_by_conversation(&mut self, conversation_id: &Id) -> Result<bool>;
    fn clear_messages(&mut self) -> Result<()>;

    fn put_friend_request(&mut self, request: &StoredFriendRequest) -> Result<()>;
    fn get_friend_request(&mut self, user_id: &Id) -> Result<Option<StoredFriendRequest>>;
    fn get_friend_requests(&mut self) -> Result<Vec<StoredFriendRequest>>;
    fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool>;
    fn remove_friend_requests(&mut self, user_ids: &[Id]) -> Result<bool>;
    fn clear_friend_requests(&mut self) -> Result<()>;

    fn get_conversation(&mut self, conversation_id: &Id) -> Result<Option<StoredConversation>>;
    fn get_all_conversations(&mut self) -> Result<Vec<StoredConversation>>;
    fn remove_conversation(&mut self, conversation_id: &Id) -> Result<bool> {
        self.remove_messages_by_conversation(conversation_id)
    }
    fn remove_conversations(&mut self, conversation_ids: &[Id]) -> Result<bool>;

    fn update_channel_ownership(
        &mut self,
        channel_id: &Id,
        old_owner_id: &Id,
        new_owner_id: &Id,
    ) -> Result<()>;
    fn put_channel_member(&mut self, channel_id: &Id, member: &StoredChannelMember) -> Result<()> {
        self.put_channel_members(channel_id, &[member.clone()])
    }
    fn put_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[StoredChannelMember],
    ) -> Result<()>;
    fn refill_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[StoredChannelMember],
    ) -> Result<()>;
    fn get_channel_member(
        &mut self,
        channel_id: &Id,
        member_id: &Id,
    ) -> Result<Option<StoredChannelMember>>;
    fn get_channel_members(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
    ) -> Result<Vec<StoredChannelMember>>;
    fn get_all_channel_members(&mut self, channel_id: &Id) -> Result<Vec<StoredChannelMember>>;
    fn update_channel_members_role(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
        role: i32,
    ) -> Result<bool>;
    fn remove_channel_member(&mut self, channel_id: &Id, member_id: &Id) -> Result<bool> {
        self.remove_channel_members(channel_id, &[*member_id])
    }
    fn remove_channel_members(&mut self, channel_id: &Id, member_ids: &[Id]) -> Result<bool>;
}
