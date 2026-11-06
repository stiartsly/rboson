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
    collections::HashMap,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use boson_api::{
    errors::StateError,
    Id,
    Result,
};

use crate::{
    channel::Channel,
    channel_member::ChannelMember,
    contact::{Contact, ContactType},
    content::{
        ContentFormat,
        MessageContent,
    },
    conversation::Conversation,
    friend_request::FriendRequest,
    internal::ContactData,
    message::{
        Content,
        Message,
    },
    MessageOps,
    persistence::{
        database::SqliteMessagingStore,
        MessagingStore,
        StoredChannel,
        StoredChannelMember,
        StoredContact,
        StoredConversation,
        StoredFriendRequest,
        StoredMessage,
    },
};

pub(crate) struct MessagingRepository {
    store: SqliteMessagingStore,
}

impl MessagingRepository {
    pub(crate) fn new() -> Self {
        Self {
            store: SqliteMessagingStore::new(),
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

    pub(crate) fn put_message(&mut self, message: &mut Message) -> Result<()> {
        let stored = message_to_stored(message)?;
        message.rid = self.store.put_message(&stored)?;
        Ok(())
    }

    pub(crate) fn update_message_sent_time(&mut self, message: &Message) -> Result<()> {
        let sent_at = message
            .sent_at
            .map(time_to_millis)
            .transpose()?
            .unwrap_or(0);
        self.store.update_message_sent_time(&message.id, sent_at)
    }

    pub(crate) fn messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>> {
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
    ) -> Result<Vec<Message>> {
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
        self.store.remove_messages_by_conversation(conversation_id)
    }

    pub(crate) fn clear_messages(&mut self) -> Result<()> {
        self.store.clear_messages()
    }

    pub(crate) fn put_contact(
        &mut self,
        revision: i32,
        contact: &Contact,
        session_key: Option<&[u8]>,
    ) -> Result<()> {
        let stored = contact_to_stored(contact, session_key);
        self.store.put_contact(revision, &stored)
    }

    pub(crate) fn put_contact_locally(
        &mut self,
        contact: &Contact,
        session_key: Option<&[u8]>,
    ) -> Result<()> {
        let stored = contact_to_stored(contact, session_key);
        self.store.put_contact_locally(&stored)
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

    pub(crate) fn put_channel(
        &mut self,
        revision: i32,
        channel: &Channel,
        session_key: &[u8],
    ) -> Result<()> {
        let stored = channel_to_stored(channel, session_key);
        self.store.put_contact(revision, &stored)?;
        let members: Vec<StoredChannelMember> = channel
            .members
            .iter()
            .map(|member| member_to_stored(channel.id(), member))
            .collect();
        self.store.refill_channel_members(channel.id(), &members)
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

    pub(crate) fn put_friend_request(&mut self, request: &FriendRequest) -> Result<()> {
        self.store
            .put_friend_request(&friend_request_to_stored(request)?)
    }

    pub(crate) fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool> {
        self.store.remove_friend_request(user_id)
    }

    pub(crate) fn clear_friend_requests(&mut self) -> Result<()> {
        self.store.clear_friend_requests()
    }

    pub(crate) fn friend_requests(&mut self) -> Result<Vec<FriendRequest>> {
        self.store
            .get_friend_requests()?
            .into_iter()
            .map(stored_to_friend_request)
            .collect()
    }

    pub(crate) fn stored_contacts(&mut self) -> Result<Vec<StoredContact>> {
        self.store.get_all_contacts()
    }

    pub(crate) fn contact_from_stored(&self, stored: &StoredContact) -> Result<Contact> {
        stored_to_contact(stored)
    }

    pub(crate) fn channel_members(&mut self, channel_id: &Id) -> Result<Vec<ChannelMember>> {
        self.store
            .get_all_channel_members(channel_id)?
            .iter()
            .map(stored_to_member)
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

    pub(crate) fn conversations(&mut self) -> Result<Vec<Conversation>> {
        self.store
            .get_all_conversations()?
            .into_iter()
            .map(stored_to_conversation)
            .collect()
    }

    pub(crate) fn conversation(&mut self, id: &Id) -> Result<Option<Conversation>> {
        self.store
            .get_conversation(id)?
            .map(stored_to_conversation)
            .transpose()
    }
}

fn time_to_millis(time: SystemTime) -> Result<i64> {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| StateError::new(format!("Invalid message timestamp: {error}")))?
        .as_millis();
    i64::try_from(millis)
        .map_err(|_| StateError::new("Message timestamp is out of range").into())
}

fn millis_to_time(millis: i64) -> Result<SystemTime> {
    let millis = u64::try_from(millis)
        .map_err(|_| StateError::new("Invalid negative message timestamp"))?;
    Ok(UNIX_EPOCH + Duration::from_millis(millis))
}

fn message_to_stored(message: &Message) -> Result<StoredMessage> {
    let payload = if !message.payload_bytes.is_empty() {
        Some(message.payload_bytes.clone())
    } else if let Some(content) = &message.content {
        Some(serialize_content(content)?)
    } else {
        None
    };

    let content_type = message
        .content
        .as_ref()
        .map(|content| content.content_type().to_string());
    let content_disposition = match message.content.as_ref() {
        Some(content) => content
            .content_disposition()?
            .map(|disposition| disposition.value().to_string()),
        None => None,
    };
    let conversation_id = message
        .conversation_id()
        .copied()
        .ok_or_else(|| StateError::new("Message has no conversation id"))?;
    let created_at = i64::try_from(message.created_at)
        .map_err(|_| StateError::new("Message timestamp is out of range"))?;
    let sent_at = message.sent_at.map(time_to_millis).transpose()?.unwrap_or(0);
    let received_at = message
        .received_at
        .map(time_to_millis)
        .transpose()?
        .unwrap_or(0);

    Ok(StoredMessage {
        rid: message.rid,
        id: message.id,
        conversation_id,
        version: i32::from(message.version),
        recipient: message.recipient,
        message_type: i32::from(message.message_type),
        from: message.from,
        created_at,
        content_type,
        content_disposition,
        payload,
        sent_at,
        received_at,
    })
}

fn serialize_content(content: &Content) -> Result<Vec<u8>> {
    let mut headers = HashMap::new();
    for (key, value) in content.headers() {
        headers.insert(
            key.clone(),
            serde_cbor::value::to_value(value).map_err(|error| {
                StateError::new(format!("Invalid message content header: {error}"))
            })?,
        );
    }
    let content = MessageContent::new(
        headers,
        ContentFormat::Binary,
        serde_cbor::Value::Bytes(content.body().to_vec()),
    )?;
    content.serialize()
}

fn stored_to_message(stored: StoredMessage) -> Result<Message> {
    let payload = stored.payload.unwrap_or_default();
    let version = u8::try_from(stored.version)
        .map_err(|_| StateError::new("Invalid message version"))?;
    let message_type = u8::try_from(stored.message_type)
        .map_err(|_| StateError::new("Invalid message type"))?;
    let created_at = u64::try_from(stored.created_at)
        .map_err(|_| StateError::new("Invalid message timestamp"))?;

    let mut message = Message::new(
        version,
        stored.id,
        stored.recipient,
        message_type,
        stored.from,
        created_at,
        serde_cbor::Value::Bytes(payload.clone()),
    );
    message.rid = stored.rid;
    message.conversation_id = Some(stored.conversation_id);
    if stored.sent_at > 0 {
        message.sent_at = Some(millis_to_time(stored.sent_at)?);
    }
    if stored.received_at > 0 {
        message.received_at = Some(millis_to_time(stored.received_at)?);
    }
    message.payload_bytes = payload;
    if !message.payload_bytes.is_empty() {
        let content = MessageContent::parse(&message.payload_bytes)?;
        let mut headers = HashMap::new();
        for (key, value) in content.headers() {
            headers.insert(
                key.clone(),
                serde_json::to_value(value).map_err(|error| {
                    StateError::new(format!("Invalid message content header: {error}"))
                })?,
            );
        }
        let body = match content.body() {
            serde_cbor::Value::Text(text) => text.as_bytes().to_vec(),
            serde_cbor::Value::Bytes(bytes) => bytes.clone(),
            _ => {
                return Err(StateError::new("Unsupported message content body").into());
            }
        };
        message.content = Some(Content::_new(headers, body));
    }
    Ok(message)
}

fn contact_to_stored(contact: &Contact, session_key: Option<&[u8]>) -> StoredContact {
    StoredContact {
        id: *contact.id(),
        contact_type: contact.contact_type() as i32,
        session_key: session_key.map(|key| key.to_vec()),
        name: contact.name().map(ToOwned::to_owned),
        remark: contact.remark().map(ToOwned::to_owned),
        tags: contact.tags().map(ToOwned::to_owned),
        muted: contact.is_muted(),
        blocked: contact.is_blocked(),
        revision: contact.revision(),
        created_at: contact.created_at(),
        updated_at: contact.updated_at(),
        channel: None,
    }
}

fn channel_to_stored(channel: &Channel, session_key: &[u8]) -> StoredContact {
    let mut stored = contact_to_stored(&channel.contact, Some(session_key));
    stored.contact_type = ContactType::Channel as i32;
    stored.channel = Some(StoredChannel {
        id: *channel.id(),
        owner: channel.owner,
        permission: channel.permission as i32,
        notice: channel.notice.clone(),
        announce: channel.announced,
    });
    stored
}

fn stored_to_contact(stored: &StoredContact) -> Result<Contact> {
    let mut data = ContactData::new(
        stored.id,
        u8::try_from(stored.contact_type)
            .map_err(|_| StateError::new("Invalid contact type"))?,
        stored.created_at,
    );
    data.name = stored.name.clone();
    data.remark = stored.remark.clone();
    data.tags = stored.tags.clone();
    data.muted = stored.muted;
    data.blocked = stored.blocked;
    data.updated_at = stored.updated_at;
    data.revision = stored.revision;
    Contact::try_from(data)
}

fn member_to_stored(channel_id: &Id, member: &ChannelMember) -> StoredChannelMember {
    StoredChannelMember {
        id: *member.id(),
        channel_id: *channel_id,
        role: member.role() as i32,
        joined: member.joined() as i64,
    }
}

fn stored_to_member(stored: &StoredChannelMember) -> Result<ChannelMember> {
    Ok(ChannelMember::new(
        stored.id,
        crate::channel::Role::try_from(stored.role)?,
        u64::try_from(stored.joined)
            .map_err(|_| StateError::new("Invalid member join timestamp"))?,
    ))
}

fn friend_request_to_stored(request: &FriendRequest) -> Result<StoredFriendRequest> {
    Ok(StoredFriendRequest {
        id: *request.user_id(),
        initiator: *request.initiator_id(),
        hello: request.hello().map(ToOwned::to_owned),
        created_at: time_to_millis(request.created_at())?,
        updated_at: time_to_millis(request.updated_at())?,
        accepted: request.is_accepted(),
        accepted_at: request
            .accepted_at()
            .map(time_to_millis)
            .transpose()?
            .unwrap_or(0),
    })
}

fn stored_to_friend_request(stored: StoredFriendRequest) -> Result<FriendRequest> {
    Ok(FriendRequest::restore(
        stored.id,
        stored.initiator,
        stored.hello,
        stored.accepted,
        millis_to_time(stored.created_at)?,
        if stored.accepted_at > 0 {
            Some(millis_to_time(stored.accepted_at)?)
        } else {
            None
        },
        millis_to_time(stored.updated_at)?,
    ))
}

fn stored_to_conversation(stored: StoredConversation) -> Result<Conversation> {
    let contact = stored_to_contact(&stored.contact)?;
    let mut conversation = Conversation::new(contact);
    if let Some(message) = stored.last_message {
        conversation.update(stored_to_message(message)?)?;
    }
    Ok(conversation)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> Id {
        let mut bytes = [0u8; 32];
        bytes[31] = value;
        Id::try_from(&bytes[..]).unwrap()
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn messaging_repository_round_trips_domain_objects() {
        let path = std::env::temp_dir().join(format!(
            "boson-messaging-repository-{}-{}.db",
            std::process::id(),
            time_to_millis(SystemTime::now()).unwrap()
        ));
        cleanup(&path);

        let mut repository = MessagingRepository::new();
        repository.open(&path).unwrap();

        let contact_id = id(1);
        let mut contact = Contact::new(contact_id, ContactType::Friend, 1);
        contact.name = Some("friend".into());
        contact.revision = 1;
        repository.put_contact(1, &contact, None).unwrap();
        let stored = repository.stored_contacts().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(
            repository.contact_from_stored(&stored[0]).unwrap().name(),
            Some("friend")
        );

        let mut message = Message::new(
            2,
            id(2),
            id(3),
            1,
            Some(contact_id),
            10,
            serde_cbor::Value::Bytes(Vec::new()),
        );
        message.conversation_id = Some(contact_id);
        message.payload_bytes = Vec::new();
        repository.put_message(&mut message).unwrap();
        assert!(message.rid > 0);
        let messages = repository.messages_before(&contact_id, i64::MAX, 10, 0).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].rid, message.rid);
        let conversation = repository.conversation(&contact_id).unwrap().unwrap();
        assert_eq!(conversation.id(), &contact_id);

        let request = FriendRequest::new(
            id(4),
            id(5),
            Some("hello".into()),
            SystemTime::UNIX_EPOCH + Duration::from_millis(10),
        );
        repository.put_friend_request(&request).unwrap();
        assert_eq!(repository.friend_requests().unwrap().len(), 1);

        repository.close();
        cleanup(&path);
    }
}
