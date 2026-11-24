
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

use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use boson_api::{
    errors::StateError,
    Id,
    Result,
};

use crate::{ChannelHandler, ConversationHandler};
use crate::internal::photon_channel::PhotonChannel;
use crate::{
    channel::ChannelMember,
    contact::{
        Contact,
        ContactType,
    },
    database::{
        database::SqliteMessagingStore,
        MessagingStore,
        StoredChannel,
        StoredChannelMember,
        StoredConversation,
        StoredMessage,
    },
    internal::photon_friend_request::PhotonFriendRequest,
    Message,
    FriendRequest,
    Conversation,
    MessageType
};

use crate::interfaces::{
    MessageContentOps,
    MessageOps,
};

use crate::internal::{
    PhotonChannelMember,
    PhotonContact,
    PhotonContent,
    PhotonConversation,
    PhotonMessage,
};

pub(crate) struct MessagingRepository {
    store: SqliteMessagingStore,
    conversations: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Id, PhotonConversation>>>,
}

impl MessagingRepository {
    pub(crate) fn new() -> Self {
        Self {
            store: SqliteMessagingStore::new(),
            conversations: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub(crate) fn open(&mut self, path: &Path) -> Result<()> {
        self.store.open(path)
    }

    pub(crate) fn close(&mut self) {
        self.store.close();
    }

    pub(crate) fn is_open(&self) -> bool {
        self.store.is_open()
    }

    pub(crate) fn contacts_revision(&mut self) -> Result<i32> {
        self.store.get_contacts_revision()
    }

    pub(crate) fn put_message(&mut self, message: &mut PhotonMessage) -> Result<()> {
        let stored = message_to_stored(message)?;
        message.set_rid(0);
        message.set_rid(self.store.put_message(&stored)?);
        Ok(())
    }

    pub(crate) fn update_message_sent_time(&mut self, message: &PhotonMessage) -> Result<()> {
        let sent_at = message.sent_at_u64().unwrap_or(0) as i64;
        self.store.update_message_sent_time(message.id(), sent_at)
    }

    pub(crate) fn messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<PhotonMessage>> {
        self.store
            .get_messages_before(conversation_id, until, limit, offset)?
            .into_iter()
            .map(stored_to_message)
            .collect()
    }

    pub(crate) fn messages_in_range(
        &mut self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<PhotonMessage>> {
        self.store
            .get_messages_in_range(conversation_id, begin, end)?
            .into_iter()
            .map(stored_to_message)
            .collect()
    }

    pub(crate) fn remove_message(&mut self, rid: i64) -> Result<bool> {
        self.store.remove_message(rid)
    }

    pub(crate) fn remove_messages(&mut self, rids: &[i64]) -> Result<bool> {
        self.store.remove_messages(rids)
    }

    pub(crate) fn remove_messages_in_conversation(&mut self, conversation_id: &Id) -> Result<bool> {
        self.conversations.lock().unwrap().remove(conversation_id);
        self.store.remove_messages_by_conversation(conversation_id)
    }

    pub(crate) fn clear_messages(&mut self) -> Result<()> {
        self.store.clear_messages()
    }

    pub(crate) fn put_contact(
        &mut self,
        revision: i32,
        contact: &PhotonContact,
        session_key: Option<&[u8]>,
    ) -> Result<()> {
        //let stored = contact_to_stored(contact, session_key);
        self.store.put_contact(revision, contact)
    }

    pub(crate) fn put_contact_locally(
        &mut self,
        contact: &PhotonContact,
        session_key: Option<&[u8]>,
    ) -> Result<()> {
        let mut contact = contact.clone();
        if let Some(session_key) = session_key {
            contact.set_session_key(Some(session_key.to_vec()));
        }
        self.store.put_contact_locally(&contact)
    }

    pub(crate) fn remove_contact(&mut self, revision: i32, contact_id: &Id) -> Result<bool> {
        self.store.remove_contact(revision, contact_id)
    }

    pub(crate) fn remove_contact_locally(&mut self, contact_id: &Id) -> Result<bool> {
        self.store.remove_contact_locally(contact_id)
    }

    pub(crate) fn remove_contacts(&mut self, revision: i32, contact_ids: &[Id]) -> Result<bool> {
        self.store.remove_contacts(revision, contact_ids)
    }

    pub(crate) fn clear_contacts(&mut self, revision: i32) -> Result<()> {
        self.store.clear_contacts(revision)
    }

    pub(crate) fn store_channel(
        &mut self,
        revision: i32,
        channel: &dyn ChannelHandler,
        session_key: &[u8],
    ) -> Result<()> {
        let stored = channel_to_photon(channel, session_key);
        self.store.put_contact(revision, &stored)?;
        let members: Vec<StoredChannelMember> = channel
            .members()
            .iter()
            .map(|member| member_to_stored(channel.id(), member))
            .collect();
        self.store.refill_channel_members(channel.id(), &members)
    }

    pub(crate) fn put_channel(&mut self, channel: &PhotonChannel) -> Result<()> {
        let session_key = channel
            .photon_contact()
            .session_key()
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        let revision = self.store.get_contacts_revision()?;
        self.store_channel(revision, channel, &session_key)
    }

    pub(crate) fn contact(&mut self, contact_id: &Id) -> Result<Option<PhotonContact>> {
        self.store.get_contact(contact_id)
    }

    pub(crate) fn contacts(&mut self) -> Result<Vec<PhotonContact>> {
        self.load_contacts()
    }

    pub(crate) fn load_channel(&mut self, channel_id: &Id) -> Result<Option<PhotonChannel>> {
        let Some(contact) = self.store.get_contact(channel_id)? else {
            return Ok(None);
        };
        let Some(metadata) = contact.channel.clone() else {
            return Ok(None);
        };
        let members = self.photon_channel_members(channel_id)?;
        Ok(Some(PhotonChannel::new(
            contact,
            metadata.owner,
            Id::default(),
            crate::channel::Permission::try_from(metadata.permission)?,
            metadata.notice,
            metadata.announce,
            members,
        )))
    }

    pub(crate) fn load_channels(&mut self) -> Result<Vec<PhotonChannel>> {
        let ids = self
            .stored_channels()?
            .into_iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        let mut channels = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(channel) = self.load_channel(&id)? {
                channels.push(channel);
            }
        }
        Ok(channels)
    }

    pub(crate) fn remove_channel(&mut self, channel_id: &Id) -> Result<()> {
        let revision = self.store.get_contacts_revision()?;
        self.remove_contact(revision, channel_id).map(|_| ())
    }

    pub(crate) fn remove_channels(&mut self) -> Result<()> {
        let ids = self
            .stored_channels()?
            .into_iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        for id in &ids {
            self.remove_channel(id)?;
        }
        Ok(())
    }

    pub(crate) fn put_conversation(&mut self, conversation: PhotonConversation) -> Result<()> {
        self.conversations
            .lock()
            .unwrap()
            .insert(*conversation.id(), conversation);
        Ok(())
    }

    pub(crate) fn conversations_handle(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Id, PhotonConversation>>> {
        self.conversations.clone()
    }

    pub(crate) fn put_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[ChannelMember],
    ) -> Result<()> {
        let stored: Vec<StoredChannelMember> = members
            .iter()
            .map(|member| member_to_stored(channel_id, member))
            .collect();
        self.store.put_channel_members(channel_id, &stored)
    }

    pub(crate) fn refill_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[ChannelMember],
    ) -> Result<()> {
        let stored: Vec<StoredChannelMember> = members
            .iter()
            .map(|member| member_to_stored(channel_id, member))
            .collect();
        self.store.refill_channel_members(channel_id, &stored)
    }

    pub(crate) fn update_channel_ownership(
        &mut self,
        channel_id: &Id,
        old_owner: &Id,
        new_owner: &Id,
    ) -> Result<()> {
        self.store
            .update_channel_ownership(channel_id, old_owner, new_owner)
    }

    pub(crate) fn update_channel_members_role(
        &mut self,
        channel_id: &Id,
        members: &[Id],
        role: i32,
    ) -> Result<bool> {
        self.store
            .update_channel_members_role(channel_id, members, role)
    }

    pub(crate) fn remove_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[Id],
    ) -> Result<bool> {
        self.store.remove_channel_members(channel_id, members)
    }

    pub(crate) fn put_friend_request(&mut self, request: &PhotonFriendRequest) -> Result<()> {
        self.store
            .put_friend_request(request)
    }

    pub(crate) fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool> {
        self.store.remove_friend_request(user_id)
    }

    pub(crate) fn clear_friend_requests(&mut self) -> Result<()> {
        self.store.clear_friend_requests()
    }

    pub(crate) fn load_friend_requests(&mut self) -> Result<Vec<PhotonFriendRequest>> {
        self.store
            .get_friend_requests()
    }

    pub(crate) fn load_friend_request(&mut self, user_id: &Id) -> Result<Option<PhotonFriendRequest>> {
        self.store
            .get_friend_request(user_id)
    }

    pub(crate) fn load_contacts(&mut self) -> Result<Vec<PhotonContact>> {
        self.store.get_all_contacts()
    }

    pub(crate) fn contact_from_stored(&self, stored: &PhotonContact) -> Result<PhotonContact> {
        Ok(stored.clone())
    }

    pub(crate) fn photon_contact_from_stored(&self, stored: &PhotonContact) -> Result<PhotonContact> {
        Ok(stored.clone())
    }

    pub(crate) fn channel_members(&mut self, channel_id: &Id) -> Result<Vec<ChannelMember>> {
        self.store
            .get_all_channel_members(channel_id)?
            .iter()
            .map(stored_to_member)
            .collect()
    }

    pub(crate) fn photon_channel_members(
        &mut self,
        channel_id: &Id,
    ) -> Result<Vec<PhotonChannelMember>> {
        self.store
            .get_all_channel_members(channel_id)?
            .iter()
            .map(stored_to_photon_member)
            .collect()
    }

    pub(crate) fn stored_channels(&mut self) -> Result<Vec<StoredChannel>> {
        Ok(self
            .store
            .get_all_contacts()?
            .into_iter()
            .filter_map(|contact| contact.channel)
            .collect())
    }

    pub(crate) fn load_conversations(&mut self) -> Result<Vec<PhotonConversation>> {
        let mut conversations = self
            .store
            .get_all_conversations()?
            .into_iter()
            .map(stored_to_photon_conversation)
            .collect::<Result<Vec<_>>>()?;
        for (id, conversation) in self.conversations.lock().unwrap().iter() {
            if !conversations.iter().any(|item| item.id() == id) {
                conversations.push(conversation.clone());
            }
        }
        Ok(conversations)
    }

    pub(crate) fn conversation(&mut self, id: &Id) -> Result<Option<PhotonConversation>> {
        if let Some(conversation) = self.conversations.lock().unwrap().get(id) {
            return Ok(Some(conversation.clone()));
        }
        self.store
            .get_conversation(id)?
            .map(stored_to_photon_conversation)
            .transpose()
    }
}

fn message_to_stored(message: &PhotonMessage) -> Result<StoredMessage> {
    let payload = if !message.payload_bytes.is_empty() {
        Some(message.payload_bytes.clone())
    } else if let Some(content) = &message.content {
        Some(content.serialize()?)
    } else {
        None
    };

    let content_type = message
        .content
        .as_ref()
        .map(|content| content.content_type().to_string());
    let content_disposition = match message.content.as_ref() {
        Some(content) => content
            .content_disposition()
            .map(|disposition| disposition.value().to_string()),
        None => None,
    };
    let conversation_id = message
        .conversation_id()
        .copied()
        .ok_or_else(|| StateError::new("Message has no conversation id"))?;
    let created_at = message.created_at_u64();
    let sent_at = message.sent_at_u64();
    let received_at = message.received_at_u64().unwrap_or(0);

    Ok(StoredMessage {
        rid: message.rid(),
        id: *message.id(),
        conversation_id,
        version: message.version(),
        recipient: *message.recipient(),
        message_type: message.message_type() as i32,
        from: message.from().cloned(),
        created_at: i64::try_from(created_at)
            .map_err(|_| StateError::new("Message timestamp is out of range"))?,
        content_type,
        content_disposition,
        payload,
        sent_at: sent_at
            .map(|timestamp| i64::try_from(timestamp)
                .map_err(|_| StateError::new("Sent timestamp is out of range")))
            .transpose()?
            .unwrap_or(0),
        received_at: i64::try_from(received_at)
            .map_err(|_| StateError::new("Received timestamp is out of range"))?,
    })
}

fn stored_to_message(stored: StoredMessage) -> Result<PhotonMessage> {
    let payload = stored.payload.unwrap_or_default();
    let version = u8::try_from(stored.version)
        .map_err(|_| StateError::new("Invalid message version"))?;

    let mut message = PhotonMessage::new(
        version as i32,
        stored.id,
        stored.recipient,
        MessageType::try_from(stored.message_type)?,
        stored.from,
        stored.created_at,
        serde_cbor::Value::Bytes(payload.clone()),
    );
    message.set_rid(stored.rid);
    message.set_converstation_id(stored.conversation_id);
    if stored.sent_at > 0 {
        message.set_sent(stored.sent_at);
    }
    if stored.received_at > 0 {
        message.set_received_at(stored.received_at);
    }
    message.payload_bytes = payload;
    if !message.payload_bytes.is_empty() {
        message.content = Some(PhotonContent::parse(&message.payload_bytes)?);
    }
    Ok(message)
}

fn contact_to_photon(contact: &Contact, session_key: Option<&[u8]>) -> PhotonContact {
    let mut photon = PhotonContact::new(
        *contact.id(),
        contact.contact_type(),
        contact.created_at(),
    );
    photon.name = contact.name().map(ToOwned::to_owned);
    photon.remark = contact.remark().map(ToOwned::to_owned);
    photon.tags = contact.tags().map(ToOwned::to_owned);
    photon.muted = contact.is_muted();
    photon.blocked = contact.is_blocked();
    photon.revision = contact.revision();
    photon.updated_at = contact.updated_at();
    if let Some(session_key) = session_key {
        photon.set_session_key(Some(session_key.to_vec()));
    }
    photon
}

fn channel_to_photon(channel: &dyn ChannelHandler, session_key: &[u8]) -> PhotonContact {
    let mut contact = PhotonContact::new(
        *channel.id(),
        ContactType::Channel,
        channel.created_at(),
    );
    contact.name = channel.name().map(ToOwned::to_owned);
    contact.remark = channel.remark().map(ToOwned::to_owned);
    contact.tags = channel.tags().map(ToOwned::to_owned);
    contact.muted = channel.is_muted();
    contact.blocked = channel.is_blocked();
    contact.revision = channel.revision();
    contact.updated_at = channel.updated_at();
    contact.set_session_key(Some(session_key.to_vec()));
    contact.channel = Some(StoredChannel {
        id: *channel.id(),
        owner: *channel.owner(),
        permission: channel.permission() as i32,
        notice: channel.notice().map(ToOwned::to_owned),
        announce: channel.is_announced(),
    });
    contact
}

fn stored_to_contact(stored: &PhotonContact) -> Result<PhotonContact> {
    Ok(stored.clone())
}

fn member_to_stored(channel_id: &Id, member: &ChannelMember) -> StoredChannelMember {
    StoredChannelMember {
        id: *member.id(),
        channel_id: *channel_id,
        role: member.role() as i32,
        joined: member
            .joined()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0),
    }
}

fn stored_to_member(stored: &StoredChannelMember) -> Result<ChannelMember> {
    Ok(Box::new(stored_to_photon_member(stored)?))
}

fn stored_to_photon_member(stored: &StoredChannelMember) -> Result<PhotonChannelMember> {
    Ok(PhotonChannelMember::new(
        stored.id,
        crate::channel::Role::try_from(stored.role)?,
        u64::try_from(stored.joined)
            .map_err(|_| StateError::new("Invalid member join timestamp"))?,
    ))
}

fn stored_to_photon_conversation(stored: StoredConversation) -> Result<PhotonConversation> {
    let contact = stored_to_contact(&stored.contact)?;
    let mut conversation = PhotonConversation::new(Box::new(contact) as Contact);
    if let Some(message) = stored.last_message {
        let message: Message = Box::new(stored_to_message(message)?);
        conversation.update(message)?;
    }
    Ok(conversation)
}
