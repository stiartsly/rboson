
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
    sync::{Arc, Mutex},
    collections::HashMap,
};

use boson_api::{
    errors::{DBError, StateError},
    Id,
    Result,
    CryptoIdentity,
};
use crate::{
    channel::ChannelMember,
    contact::ContactType,
    FriendRequestHandler,
    ChannelHandler,
    ChannelMemberHandler,
    ContactHandler,
    ConversationHandler,
    MessageType,
    database::{
        MessagingStore,
        SqliteStore,
        RecordChannelMember,
        RecordContact,
        RecordConversation,
        RecordFriendRequest,
        RecordMessage,
    },
};

use crate::{
    MessageContentHandler,
    MessageHandler,
};
use crate::internal::{
    PhotonChannelMember,
    PhotonContact,
    PhotonContent,
    PhotonConversation,
    PhotonMessage,
    PhotonFriendRequest,
    PhotonChannel,
    SessionContext,
    photon_contact::ChannelMetadata,
};

pub(crate) struct MessagingRepository {
    store: SqliteStore,
    user_identity: Arc<CryptoIdentity>,
    conversations: Arc<Mutex<HashMap<Id, PhotonConversation>>>,
}

impl MessagingRepository {
    pub(crate) fn new(user_identity: Arc<CryptoIdentity>) -> Self {
        Self {
            store: SqliteStore::new(),
            user_identity,
            conversations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn session_context_for(&self, contact: &PhotonContact) -> Result<SessionContext> {
        let session = contact
            .session(&self.user_identity)?
            .ok_or_else(|| StateError::new("Contact session key is missing"))?;
        let session = Arc::new(session);
        Ok(if contact.contact_type == ContactType::Channel as i32 {
            SessionContext::for_channel(*contact.id(), self.user_identity.clone(), session)
        } else {
            SessionContext::for_user(*contact.id(), self.user_identity.clone(), session)
        })
    }

    fn conversation_from_record(&self, stored: RecordConversation) -> Result<PhotonConversation> {
        let contact_record = RecordContact {
            id: stored.id,
            contact_type: stored.contact_type,
            session_key: stored.session_key,
            name: stored.name,
            remark: stored.remark,
            tags: stored.tags,
            muted: stored.muted,
            blocked: stored.blocked,
            revision: stored.revision,
            created_at: stored.created_at,
            updated_at: stored.updated_at,
            owner: stored.owner,
            permission: stored.permission,
            notice: stored.notice,
            announce: stored.announce,
        };
        let contact: PhotonContact = contact_record.into();
        let session_context = self.session_context_for(&contact)?;
        let message = RecordMessage {
            rid: stored.last_message_rid,
            id: stored.last_message_id,
            conversation_id: stored.last_message_conversation_id,
            version: stored.last_message_version,
            recipient: stored.last_message_recipient,
            message_type: stored.last_message_type,
            from_id: stored.last_message_from_id,
            created_at: stored.last_message_created_at,
            content_type: stored.last_message_content_type,
            content_disposition: stored.last_message_content_disposition,
            payload: stored.last_message_payload,
            sent_at: stored.last_message_sent_at,
            received_at: stored.last_message_received_at,
        };
        Ok(PhotonConversation::with_message(
            contact,
            Some(Box::new(PhotonMessage::try_from(message)?)),
            session_context,
        ))
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

    pub(crate) fn put_message(&mut self, message: &mut PhotonMessage) -> Result<()> {
        let record: RecordMessage = RecordMessage::from(&*message);
        let result = self.store.put_message(record)
            .map_err(|e| DBError::new(format!("Put message failed {e}")))?;

        message.set_rid(result);
        Ok(())
    }

    pub(crate) fn update_message_sent_time(&mut self, message: &PhotonMessage) -> Result<()> {
        self.store.update_message_sent_time(
            message.id(),
            message.sent_ms().unwrap_or(0)
        )
    }

    pub(crate) fn messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<PhotonMessage>> {
        Ok(self.store
            .get_messages_before(conversation_id, until, limit, offset)?
            .into_iter()
            .map(|record| {
                let message: PhotonMessage = record.into();
                message
            })
            .collect())
    }

    pub(crate) fn messages_in_range(
        &mut self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<PhotonMessage>> {
        Ok(self.store
            .get_messages_in_range(conversation_id, begin, end)?
            .into_iter()
            .map(|record| {
                let message: PhotonMessage = record.into();
                message
            })
            .collect())
    }

    pub(crate) fn remove_message(&mut self, rid: i64) -> Result<bool> {
        self.store.remove_message(rid)
    }

    pub(crate) fn remove_messages(&mut self, rids: &[i64]) -> Result<bool> {
        self.store.remove_messages(rids)
    }

    pub(crate) fn remove_messages_in_conversation(&mut self, conversation_id: &Id) -> Result<bool> {
        self.remove_messages_by_conversation(conversation_id)
    }

    pub(crate) fn remove_messages_by_conversation(&mut self, conversation_id: &Id) -> Result<bool> {
        self.conversations.lock().unwrap().remove(conversation_id);
        self.store.remove_messages_by_conversation(conversation_id)
    }

    pub(crate) fn clear_messages(&mut self) -> Result<()> {
        self.store.clear_messages()
    }

    pub(crate) fn load_friend_request(&mut self, user_id: &Id) -> Result<Option<PhotonFriendRequest>> {
        Ok(self.store.get_friend_request(user_id)?.map(Into::into))
    }

    pub(crate) fn load_friend_requests(&mut self) -> Result<Vec<PhotonFriendRequest>> {
        Ok(
            self.store
                .get_friend_requests()?
                .into_iter()
                .map(Into::into)
                .collect()
        )
    }

    pub(crate) fn put_friend_request(&mut self, request: &PhotonFriendRequest) -> Result<()> {
        self.store.put_friend_request(RecordFriendRequest::from(request))
    }

    pub(crate) fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool> {
        self.store.remove_friend_request(user_id)
    }

    pub(crate) fn clear_friend_requests(&mut self) -> Result<()> {
        self.store.clear_friend_requests()
    }

    pub(crate) fn contacts_revision(&mut self) -> Result<i32> {
        self.store.get_contacts_revision()
    }

    pub(crate) fn load_contact(&mut self, contact_id: &Id) -> Result<Option<PhotonContact>> {
        Ok(self.store.get_contact(contact_id)?.map(Into::into))
    }

    pub(crate) fn load_contacts(&mut self) -> Result<Vec<PhotonContact>> {
        Ok(
            self.store
                .get_all_contacts()?
                .into_iter()
                .map(Into::into)
                .collect()
        )
    }

    pub(crate) fn contact(&mut self, contact_id: &Id) -> Result<Option<PhotonContact>> {
        self.load_contact(contact_id)
    }

    pub(crate) fn contacts(&mut self) -> Result<Vec<PhotonContact>> {
        self.load_contacts()
    }

    pub(crate) fn put_contact(
        &mut self,
        revision: i32,
        contact: &PhotonContact,
        session_key: Option<&[u8]>,
    ) -> Result<()> {
        let mut record: RecordContact = contact.into();
        record.session_key = session_key.map(ToOwned::to_owned);
        self.store.put_contact(revision, record)
    }

    pub(crate) fn put_contact_locally(
        &mut self,
        contact: &PhotonContact,
        session_key: Option<&[u8]>,
    ) -> Result<()> {
        let mut record: RecordContact = contact.into();
        record.session_key = session_key.map(ToOwned::to_owned);
        self.store.put_contact_locally(record)
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

    pub(crate) fn load_channel(&mut self, channel_id: &Id) -> Result<Option<PhotonChannel>> {
        let Some(contact) = self.store.get_contact(channel_id)? else {
            return Ok(None);
        };
        let members = self.store.get_all_channel_members(channel_id)?;
        Ok(Some((contact, members).try_into()?))
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

    pub(crate) fn put_channel(
        &mut self,
        revision: i32,
        channel: &PhotonChannel
    ) -> Result<()> {
        let (record_contact, record_members): (RecordContact, Vec<RecordChannelMember>) = channel.try_into()?;
        self.store.put_contact(revision, record_contact)?;
        self.store.refill_channel_members(&channel.contact().id, &record_members)?;
        Ok(())
    }

    pub(crate) fn put_channel_locally(
        &mut self,
        channel: &PhotonChannel
    ) -> Result<()> {
        let (record_contact, record_members): (RecordContact, Vec<RecordChannelMember>) = channel.try_into()?;
        self.store.put_contact_locally(record_contact)?;
        self.store.refill_channel_members(&channel.contact().id, &record_members)?;
        Ok(())
    }

    pub(crate) fn remove_channel(
        &mut self,
        revision: i32,
        channel_id: &Id
    ) -> Result<()> {
        self.remove_contact(revision, channel_id).map(|_| ())
    }

    pub(crate) fn remove_channel_locally(&mut self, channel_id: &Id) -> Result<()> {
        self.store.remove_contact_locally(channel_id).map(|_| ())
    }

    pub(crate) fn remove_channels(&mut self, revision: i32) -> Result<()> {
        let ids = self
            .stored_channels()?
            .into_iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        for id in &ids {
            self.remove_channel(revision, id)?;
        }
        Ok(())
    }

    pub(crate) fn load_channel_members(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
    ) -> Result<Vec<PhotonChannelMember>> {
        Ok(self.store
            .get_channel_members(channel_id, member_ids)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub(crate) fn load_channel_member(
        &mut self,
        channel_id: &Id,
        member_id: &Id,
    ) -> Result<Option<PhotonChannelMember>> {
        Ok(self.store
            .get_channel_member(channel_id, member_id)?
            .map(Into::into))
    }

    pub(crate) fn put_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[PhotonChannelMember],
    ) -> Result<()> {
        let record: Vec<RecordChannelMember> = members
            .iter()
            .map(|member| (member, channel_id).into())
            .collect();
        self.store.put_channel_members(channel_id, record)
    }

    pub(crate) fn refill_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[PhotonChannelMember],
    ) -> Result<()> {
        let record: Vec<RecordChannelMember> = members
            .iter()
            .map(|member| (member, channel_id).into())
            .collect();
        self.store.refill_channel_members(channel_id, &record)
    }

    pub(crate) fn update_channel_ownership(
        &mut self,
        channel_id: &Id,
        old_owner: &Id,
        new_owner: &Id,
    ) -> Result<()> {
        self.store.update_channel_ownership(channel_id, old_owner, new_owner)
    }

    pub(crate) fn update_channel_members_role(
        &mut self,
        channel_id: &Id,
        members: &[Id],
        role: i32,
    ) -> Result<bool> {
        self.store.update_channel_members_role(channel_id, members, role)
    }

    pub(crate) fn remove_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[Id],
    ) -> Result<bool> {
        self.store.remove_channel_members(channel_id, members)
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

    pub(crate) fn contact_from_stored(&self, stored: &PhotonContact) -> Result<PhotonContact> {
        Ok(stored.clone())
    }

    pub(crate) fn photon_contact_from_stored(&self, stored: &PhotonContact) -> Result<PhotonContact> {
        Ok(stored.clone())
    }

    pub(crate) fn channel_members(&mut self, channel_id: &Id) -> Result<Vec<ChannelMember>> {
        Ok(self.store
            .get_all_channel_members(channel_id)?
            .into_iter()
            .map(|member| {
                let member: PhotonChannelMember = member.into();
                Box::new(member) as ChannelMember
            })
            .collect())
    }

    pub(crate) fn photon_channel_members(
        &mut self,
        channel_id: &Id,
    ) -> Result<Vec<PhotonChannelMember>> {
        Ok(self.store
            .get_all_channel_members(channel_id)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub(crate) fn stored_channels(&mut self) -> Result<Vec<ChannelMetadata>> {
        self.store
            .get_all_contacts()?
            .into_iter()
            .filter(|contact| contact.contact_type == ContactType::Channel as i32)
            .map(|contact| {
                let contact: PhotonContact = contact.into();
                contact.channel.ok_or_else(|| {
                    StateError::new("Channel metadata is missing").into()
                })
            })
            .collect()
    }

    pub(crate) fn load_conversations(&mut self) -> Result<Vec<PhotonConversation>> {
        let mut conversations = self
            .store
            .get_all_conversations()?
            .into_iter()
            .map(|record| self.conversation_from_record(record))
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
            .map(|record| self.conversation_from_record(record))
            .transpose()
    }
}

fn identifier(bytes: &[u8]) -> Id {
    Id::try_from_bytes(bytes).expect("Invalid stored identifier")
}

impl From<RecordFriendRequest> for PhotonFriendRequest {
    fn from(record: RecordFriendRequest) -> Self {
        Self::restore(
            identifier(&record.id),
            identifier(&record.initiator),
            record.hello.clone(),
            record.accepted != 0,
            record.created_at,
            if record.accepted_at != 0 { Some(record.accepted_at) } else { None },
            record.updated_at,
        )
    }
}

impl From<&PhotonFriendRequest> for RecordFriendRequest {
    fn from(fr: &PhotonFriendRequest) -> Self {
        Self {
            id: fr.user_id().as_bytes().to_vec(),
            initiator: fr.initiator_id().as_bytes().to_vec(),
            hello: fr.hello().map(ToOwned::to_owned),
            created_at: fr.created_at() as i64,
            updated_at: fr.updated_at() as i64,
            accepted: if fr.is_accepted() { 1 } else { 0 },
            accepted_at: fr.accepted_at().unwrap_or(0) as i64,
        }
    }
}

impl From<RecordContact> for PhotonContact {
    fn from(record: RecordContact) -> Self {
        let channel = if record.contact_type == ContactType::Channel as i32 {
            Some(ChannelMetadata {
                id: identifier(&record.id),
                owner: identifier(record.owner.as_deref().unwrap()),
                permission: record.permission.unwrap(),
                notice: record.notice.clone(),
                announce: record.announce.unwrap_or(0) != 0,
            })
        } else {
            None
        };
        Self {
            id          : identifier(&record.id),
            contact_type: record.contact_type,
            session_key : record.session_key,
            name        : record.name,
            remark      : record.remark,
            tags        : record.tags,
            muted       : record.muted != 0,
            blocked     : record.blocked != 0,
            revision    : record.revision,
            created_at  : record.created_at,
            updated_at  : record.updated_at,
            channel,
        }
    }
}

impl From<&PhotonContact> for RecordContact {
    fn from(contact: &PhotonContact) -> Self {
        let channel = contact.channel.as_ref();
        Self {
            id          : contact.id.as_bytes().to_vec(),
            contact_type: contact.contact_type as i32,
            session_key : contact.session_key.clone(),
            name        : contact.name.clone(),
            remark      : contact.remark.clone(),
            tags        : contact.tags.clone(),
            muted       : if contact.muted { 1 } else { 0 },
            blocked     : if contact.blocked { 1 } else { 0 },
            revision    : contact.revision,
            created_at  : contact.created_at,
            updated_at  : contact.updated_at,
            owner       : channel.map(|v| v.owner.as_bytes().to_vec()),
            permission  : channel.map(|v| v.permission),
            notice      : channel.and_then(|v| v.notice.clone()),
            announce    : channel.map(|v| if v.announce { 1 } else { 0 }),
        }
    }
}

impl From<RecordChannelMember> for PhotonChannelMember {
    fn from(record: RecordChannelMember) -> Self {
        Self::new(
            identifier(&record.id),
            crate::channel::Role::from_i32(record.role),
            record.joined as u64,
        )
    }
}

impl From<(&PhotonChannelMember, &Id)> for RecordChannelMember {
    fn from((member, channel_id): (&PhotonChannelMember, &Id)) -> Self {
        Self {
            id          : member.id().as_bytes().to_vec(),
            channel_id  : channel_id.as_bytes().to_vec(),
            role        : member.role() as i32,
            joined      : boson_api::as_ms!(member.joined()) as i64,
        }
    }
}

impl TryFrom<(RecordContact, Vec<RecordChannelMember>)> for PhotonChannel {
    type Error = boson_api::Error;

    fn try_from(recrod: (RecordContact, Vec<RecordChannelMember>)) -> Result<Self> {
        let mut contact: PhotonContact = recrod.0.into();
        let Some(metadata) = contact.channel.take() else {
            return Err(DBError::new("Missing channel metadata, not channel contact"));
        };
        let members = recrod.1.into_iter().map(Into::into).collect();
        let mut channel = Self::new(
            contact,
            metadata.owner,
            crate::channel::Permission::from_i32(metadata.permission),
            metadata.notice,
            metadata.announce,
        );
        channel.set_channel_members(members)?;
        Ok(channel)
    }
}

impl TryFrom<&PhotonChannel> for (RecordContact, Vec<RecordChannelMember>) {
    type Error = boson_api::Error;

    fn try_from(channel: &PhotonChannel) -> Result<Self> {
        let mut contact = RecordContact::from(&channel.photon_contact());
        contact.contact_type = ContactType::Channel as i32;
        contact.owner = Some(channel.owner().as_bytes().to_vec());
        contact.permission = Some(channel.permission() as i32);
        contact.notice = channel.notice().map(ToOwned::to_owned);
        contact.announce = Some(if channel.is_announced() { 1 } else { 0 });
        let members = channel
            .photon_members()
            .into_iter()
            .map(|member| RecordChannelMember::from((&member, &channel.contact().id)))
            .collect();
        Ok((contact, members))
    }
}

impl From<&PhotonMessage> for RecordMessage {
    fn from(message: &PhotonMessage) -> Self {
        let payload = if !message.payload_bytes.is_empty() {
            Some(message.payload_bytes.clone())
        } else if let Some(content) = &message.content {
            Some(content.serialize().unwrap())
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
            .unwrap();
        let created_at = message.created_ms();
        let sent_at = message.sent_ms();
        let received_at = message.received_ms().unwrap_or(0);
        Self {
            rid: message.rid(),
            id: message.id().as_bytes().to_vec(),
            conversation_id: conversation_id.as_bytes().to_vec(),
            version: message.version(),
            recipient: message.recipient().as_bytes().to_vec(),
            message_type: message.message_type() as i32,
            from_id: message.from().map(|id| id.as_bytes().to_vec()),
            created_at: created_at,
            content_type,
            content_disposition,
            payload,
            sent_at: sent_at.unwrap_or(0),
            received_at,
        }
    }
}

impl From<RecordMessage> for PhotonMessage {
    fn from(record: RecordMessage) -> Self {
        let payload = record.payload.unwrap_or_default();
        let mut message = PhotonMessage::restore(
            record.rid,
            Some(identifier(&record.conversation_id)),
            record.version,
            identifier(&record.id),
            identifier(&record.recipient),
            MessageType::try_from(record.message_type).unwrap(),
            record.from_id.as_deref().map(identifier),
            record.created_at,
            serde_cbor::Value::Bytes(payload.clone()),
            record.sent_at,
            record.received_at,
        ).unwrap();
        message.payload_bytes = payload;
        if !message.payload_bytes.is_empty() {
            message.content = Some(PhotonContent::parse(&message.payload_bytes).unwrap());
        }
        message
    }
}
