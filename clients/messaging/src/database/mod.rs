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

use std::path::Path;

mod sqlite;
mod dialect;

use boson_api::{Id, Result};
pub(crate) use sqlite::{
    SqliteStore,
    RecordChannelMember,
    RecordContact,
    RecordConversation,
    RecordFriendRequest,
    RecordMessage,
};

#[allow(dead_code)]
pub(crate) trait MessagingStore {
    fn open(&mut self, path: &Path) -> Result<()>;
    fn close(&mut self);
    fn is_open(&self) -> bool;

    fn get_contacts_revision(&mut self) -> Result<i32>;

    fn get_friend_request(&mut self, user_id: &Id) -> Result<Option<RecordFriendRequest>>;
    fn get_friend_requests(&mut self) -> Result<Vec<RecordFriendRequest>>;
    fn put_friend_request(&mut self, request: RecordFriendRequest) -> Result<()>;
    fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool>;
    fn remove_friend_requests(&mut self, user_ids: &[Id]) -> Result<bool>;
    fn clear_friend_requests(&mut self) -> Result<()>;

    fn get_contact(&mut self, contact_id: &Id) -> Result<Option<RecordContact>>;
    fn get_contacts(&mut self, contact_ids: &[Id]) -> Result<Vec<RecordContact>>;
    fn get_all_contacts(&mut self) -> Result<Vec<RecordContact>>;
    fn exists_contact(&mut self, contact_id: &Id) -> Result<bool>;
    fn put_contact(&mut self, revision: i32, contact: RecordContact) -> Result<()>;
    fn put_contacts(&mut self, revision: i32, contacts: Vec<RecordContact>) -> Result<()>;
    fn put_contact_locally(&mut self, contact: RecordContact) -> Result<()>;
    fn remove_contact_locally(&mut self, contact_id: &Id) -> Result<bool>;
    fn remove_contact(&mut self, revision: i32, contact_id: &Id) -> Result<bool>;
    fn remove_contacts(&mut self, revision: i32, contact_ids: &[Id]) -> Result<bool>;
    fn clear_contacts(&mut self, revision: i32) -> Result<()>;

    fn update_channel_ownership(
        &mut self,
        channel_id: &Id,
        old_owner_id: &Id,
        new_owner_id: &Id,
    ) -> Result<()>;
    fn put_channel_member(
        &mut self,
        channel_id: &Id,
        member: RecordChannelMember
    ) -> Result<()> {
        self.put_channel_members(channel_id, vec![member])
    }
    fn put_channel_members(
        &mut self,
        channel_id: &Id,
        members: Vec<RecordChannelMember>,
    ) -> Result<()>;
    fn refill_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[RecordChannelMember],
    ) -> Result<()>;
    fn get_channel_member(
        &mut self,
        channel_id: &Id,
        member_id: &Id,
    ) -> Result<Option<RecordChannelMember>>;
    fn get_channel_members(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
    ) -> Result<Vec<RecordChannelMember>>;
    fn get_all_channel_members(
        &mut self,
        channel_id: &Id
    ) -> Result<Vec<RecordChannelMember>>;
    fn update_channel_members_role(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
        role: i32,
    ) -> Result<bool>;
    fn remove_channel_member(
        &mut self,
        channel_id: &Id,
        member_id: &Id
    ) -> Result<bool> {
        self.remove_channel_members(channel_id, &[*member_id])
    }
    fn remove_channel_members(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id]
    ) -> Result<bool>;
    fn get_conversation(
        &mut self,
        conversation_id: &Id
    ) -> Result<Option<RecordConversation>>;
    fn get_all_conversations(&mut self) -> Result<Vec<RecordConversation>>;
    fn remove_conversation(
        &mut self,
        conversation_id: &Id
    ) -> Result<bool> {
        self.remove_messages_by_conversation(conversation_id)
    }
    fn remove_conversations(
        &mut self,
        conversation_ids: &[Id]
    ) -> Result<bool>;

    fn put_message(&mut self, message: RecordMessage) -> Result<i64>;
    fn update_message_sent_time(
        &mut self,
        message_id: &Id,
        sent_at: i64
    ) -> Result<()>;
    fn get_messages_in_range(
        &mut self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<RecordMessage>>;
    fn get_messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<RecordMessage>>;
    fn remove_message(&mut self, rid: i64) -> Result<bool> {
        self.remove_messages(&[rid])
    }
    fn remove_messages(&mut self, rids: &[i64]) -> Result<bool>;
    fn remove_messages_by_conversation(&mut self, conversation_id: &Id) -> Result<bool>;
    fn clear_messages(&mut self) -> Result<()>;
}
