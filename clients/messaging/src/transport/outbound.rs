pub(crate) use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    sync::Mutex,
    time::{Duration, SystemTime},
};
pub(crate) use serde_cbor::Value;
use log::{warn, info, debug, error};

use super::mqtt;


pub(crate) use boson_api::{
    Result,
    errors::{ArgumentError, StateError},
    signature,
    CryptoIdentity,
    Id,
    Identity,
};

use crate::{
    ChannelHandler,
    channel::{Channel, Permission, Role},
    contact::{Contact, ContactType, ContactHandler},
    content_disposition::ContentDisposition,
    content_type,
    errors::{
        AuthenticationError,
        EncodingError,
    },
    message::MessageType,
    session_info::SessionInfo,
    FriendRequestHandler,
    InviteTicket,
    Message,
};

pub(crate) use crate::MessageBuilderHandler;

pub(crate) use crate::internal::{
    ChannelSessionKeyRotation,
    ContactMutation,
    ContentFormat,
    ChannelInfoUpdate,
    PhotonFriendRequest,
    Handshake,
    NewChannelInfo,
    PhotonChannel,
    PhotonContact,
    PhotonContent,
    PhotonMessage,
    PhotonConversation,
    PhotonFriend,
    SessionContext,
};

use crate::rpc::{
    response,
    response::Value as RValue,
    error::RPCError,
    request::RPCRequest,
};

pub(crate) use super::mqtt::ClientHandle;
pub(crate) use super::{
    RpcServer,
    MESSAGE_VERSION,
    RPC_TIMEOUT
};

impl RpcServer {
    pub(crate) async fn get_conversation(
        &self,
        id: &Id,
    ) -> Result<Option<PhotonConversation>> {
        Ok(self.conversation(id).await)
    }

    pub(crate) async fn get_conversation_list(
        &self,
    ) -> Result<Vec<PhotonConversation>> {
        Ok(self.conversations().await)
    }

    pub(crate) async fn remove_conversation(&self, id: &Id) -> Result<()> {
        self.repository
            .borrow_mut()
            .remove_messages_in_conversation(id)
            .map(|_| ())
    }

    pub(crate) async fn remove_conversation_list(&self, ids: &[Id]) -> Result<()> {
        for id in ids {
            self.remove_conversation(id).await?;
        }
        Ok(())
    }

    /// Reads a page of history. Not implemented yet.
    pub(crate) async fn get_messages(
        &self,
        conversation_id: &Id,
        until: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>> {
        let messages = self.repository.borrow_mut().messages_before(
            conversation_id,
            until.unwrap_or(i64::MAX),
            limit,
            offset,
        )?;
        Ok(messages.into_iter().map(|message| Box::new(message) as Message).collect())
    }

    /// Reads history in a timestamp range. Not implemented yet.
    pub(crate) async fn get_messages_in_range(
        &self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<Message>> {
        let messages = self.repository
            .borrow_mut()
            .messages_in_range(conversation_id, begin, end)?;
        Ok(messages.into_iter().map(|message| Box::new(message) as Message).collect())
    }

    /// Removes a message. Not implemented yet.
    pub(crate) async fn _remove_message(&self, message_id: i64) -> Result<()> {
        self.repository.borrow_mut().remove_message(message_id)?;
        Ok(())
    }

    /// Removes messages by id. Not implemented yet.
    pub(crate) async fn remove_messages_by_ids(&self, message_ids: &[i64]) -> Result<()> {
        self.repository.borrow_mut().remove_messages(message_ids)?;
        Ok(())
    }

    /// Clears a conversation's history. Not implemented yet.
    pub(crate) async fn remove_messages_by_conversation(
        &self,
        conversation_id: &Id,
    ) -> Result<()> {
        self.repository
            .borrow_mut()
            .remove_messages_in_conversation(conversation_id)?;
        Ok(())
    }

    pub(crate) async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        let request = RPCRequest::list_sessions(0);

        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::SessionList(sessions) = result else {
            return Err(EncodingError::new("RPC result is not a session list").into());
        };

        Ok(sessions)
    }

    pub(crate) async fn revoke_session(&self, device_id: &Id) -> Result<()> {
        assert!(device_id != self.device_id());

        let request = RPCRequest::revoke_session(0, *device_id);
        let _ = self.send_rpccall(self.peer_id(), request).await?;
        Ok(())
    }

    pub(crate) async fn send_friend_request(
        &self,
        user_id: &Id,
        hello: Option<String>
    ) -> Result<()> {
        assert!(
            user_id != self.user_identity.id(),
            "Cannot send a friend request to yourself"
        );

        let timestamp = self.origin_timestamp();
        let handshake = Handshake::friend_request(hello.clone(), timestamp);
        let payload = handshake.serialize();

        let fr = PhotonFriendRequest::new(
            *user_id,
            *self.user_identity.id(),
            hello,
            timestamp,
        );

        // TODO: cache to repository.
        self.put_friend_request(&fr)?;

        let message_id  = Self::message_id(self.device_id(), timestamp)?;
        let message = PhotonMessage::outgoing(
            message_id,
            *user_id,
            MessageType::Handshake,
            timestamp,
            Value::Bytes(payload.clone()),
        );

        info!("Sending friend request to {user_id}");

        if let Err(error) = self.send_message_internal(message).await {
            error!("Failed to send friend request to {user_id}: {error}");
            // TODO: remove the cached friend_request.
            let _ = self.remove_friend_request(user_id);
            return Err(error.into());
        }

        info!("Friend request successfully sent to {user_id}");
        Ok(())
    }

    pub(crate) async fn accept_friend_request(&self, user_id: &Id) -> Result<()> {
        let Some(mut fr) = self.friend_request(user_id) else {
            return Err(StateError::new(
                format!("No friend request found from {user_id}")));
        };
        if fr.initiator_id() == self.user_id() {
            return Err(StateError::new(
                "Can not accept the friend request from yourself"));
        }
        if fr.is_accepted() {
            return Err(StateError::new(
                "Friend request has already been accepted"));
        }
        if fr.is_expired() {
            return Err(StateError::new("Friend request has expired"));
        }

        if self.get_contact(user_id).await
            .is_some_and(|v| v.is_blocked() || v.contact_type() == ContactType::Channel)
        {
            return Err(StateError::new(
                "Cannot accept a friend request from a blocked user or channel",
            ));
        }

        let session_key = signature::KeyPair::random();
        let session_sk = session_key.private_key().as_bytes().to_vec();
        let timestamp = self.origin_timestamp();
        let handshake = Handshake::friend_request_accept(session_sk.clone(), timestamp);
        let payload = handshake.serialize();
        let message = PhotonMessage::outgoing(
            Self::message_id(self.device_id(), timestamp)?,
            *user_id,
            MessageType::Handshake,
            timestamp,
            Value::Bytes(payload.clone()),
        );

        debug!("Sending friend request acceptance to {user_id}");
        if let Err(error) = self.send_message_internal(message).await {
            error!("Failed to send friend request accept to {user_id}: {error}");
            return Err(error.into());
        }
        debug!("Friend request acceptance successfully sent to {user_id}");

        let now = boson_api::as_ms!(SystemTime::now()) as i64;
        fr.accept(now);
        info!("Accepted friend request from {user_id}");

        self.put_friend_request(&fr)?;
        debug!("Friend request accepted, adding to contacts");

        let encrypted_sk = self.self_context
            .borrow_mut()
            .encrypt_into(&session_sk)
           .map_err(|error| StateError::new(
                &format!("Failed to encrypt session key: {error}")))?;

        self.add_friend_internal(*user_id, encrypted_sk, None).await?;

        let session_identity = CryptoIdentity::from(session_key);
        self.session_contexts.lock().unwrap().insert(
            *user_id,
            self.session_context(*user_id, session_identity, false),
        );
        self.friend_request_listener.on_friend_request_accepted(user_id);
        Ok(())
    }

    pub(crate) fn friend_request(&self, id: &Id) -> Option<PhotonFriendRequest> {
        self.repository
            .borrow_mut()
            .load_friend_request(id)
            .unwrap_or_default()
    }

    pub(crate) fn friend_requests(&self) -> Vec<PhotonFriendRequest> {
        self.repository
            .borrow_mut()
            .load_friend_requests()
            .unwrap_or_default()
    }

    pub(crate) fn remove_friend_request(&self, user_id: &Id) -> Result<()> {
        self.repository
            .borrow_mut()
            .remove_friend_request(user_id)
            .map(|_| ())
    }

    pub(crate) fn remove_friend_requests(&self, user_ids: &[Id]) -> Result<()> {
        for user_id in user_ids {
            self.remove_friend_request(user_id)?;
        }
        Ok(())
    }

    pub(crate) async fn add_friend(
        &self,
        user_id: Id,
        session_key: Vec<u8>,
        remark: Option<String>,
    ) -> Result<()> {

        assert!(
            &user_id != self.user_id(),
            "Cannot add yourself as a friend",
        );

        let session_identity = CryptoIdentity::try_from(session_key.as_slice())?;
        let encrypted_sk = self.self_context.borrow_mut().encrypt_into(session_key.as_slice())
            .map_err(|error| StateError::new(
                &format!("Failed to encrypt session key: {error}")))?;

        self.add_friend_internal(user_id, encrypted_sk, remark).await?;
        self.session_contexts.lock().unwrap().insert(
            user_id,
            self.session_context(user_id, session_identity, false),
        );
        Ok(())
    }

    pub(super) async fn add_friend_internal(
        &self,
        id: Id,
        session_key: Vec<u8>,
        remark: Option<String>
    ) -> Result<()> {
        let revision = self.contacts_revision.get();

        if let Some(existing) = self.get_contact(&id).await {
            if existing.contact_type() == ContactType::Auto {
                // A user known only as an AUTO contact becomes the friend in place,
                // keeping the existing remark/tags/muted/blocked state.
                let now = boson_api::as_ms!(SystemTime::now()) as i64;
                let contact = existing.edit()
                    .set_session_key(Some(session_key))
                    .set_remark(remark.or(existing.remark))
                    .set_updated_at(now.max(existing.updated_at))
                    .set_revision(revision)
                    .build();
                let friend = PhotonFriend::from(PhotonContact::from_contact(&contact));
                let mutation = ContactMutation::update(revision, friend.to_opaque(self.self_context.clone())?);
                return self.add_friend_internal_with_mutation(friend, mutation).await;
            }
        }

        let friend = PhotonFriend::with_remark(id, session_key, remark);
        let mutation = ContactMutation::add(revision, friend.to_opaque(self.self_context.clone())?);
        self.add_friend_internal_with_mutation(friend, mutation).await
    }

    async fn add_friend_internal_with_mutation(
        &self,
        mut friend: PhotonFriend,
        mutation: ContactMutation,
    ) -> Result<()> {
        let request = RPCRequest::contact_mutate(0, mutation);
        let response = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = response else {
            return Err(ArgumentError::new("Unexpected response type").into());
        };

        friend.set_revision(revision);
        self.contacts_revision.set(revision);
        self.put_contact(friend.contact()).await?;
        Ok(())
    }

    /// Blocks a user by marking their contact as blocked and syncing the change.
    pub(crate) async fn block_user(&self, user_id: &Id) -> Result<Contact> {
        assert!(
            user_id != self.user_id(),
            "Cannot block yourself",
        );

        let revision = self.contacts_revision.get();
        let now = boson_api::as_ms!(SystemTime::now()) as i64;

        if let Some(existing) = self.get_contact(user_id).await {
            if existing.contact_type() == ContactType::Channel {
                return Err(ArgumentError::new(format!(
                    "Not a user: {user_id} is a channel"
                ))
                .into());
            }
            if existing.is_blocked() {
                self.blocked_contacts.lock().unwrap().insert(*user_id);
                return Ok(Box::new(existing) as Contact);
            }

            let blocked = existing.edit()
                .set_blocked(true)
                .set_updated_at(now.max(existing.updated_at))
                .set_revision(revision)
                .build();

            let mut existing = PhotonContact::from_contact(&blocked);
            let opaque = existing.to_opaque_document(&self.user_identity)?;
            let mutation = ContactMutation::update(revision, opaque);
            let request = RPCRequest::contact_mutate(0, mutation);
            let result = self.send_rpccall(self.peer_id(), request).await?;
            let response::Value::ContactMutate(confirmed) = result else {
                return Err(ArgumentError::new("Unexpected response type").into());
            };

            existing.set_revision(confirmed);
            self.contacts_revision.set(confirmed);
            self.put_contact(&existing).await?;
            self.blocked_contacts.lock().unwrap().insert(*user_id);
            return Ok(Box::new(existing) as Contact);
        }

        // Not a contact: an AUTO contact carries the blocked state. It has no
        // session key, so it can never be mistaken for a friend.
        let blocked = PhotonContact::new(*user_id, ContactType::Auto, now)
            .edit()
            .set_blocked(true)
            .set_updated_at(now)
            .set_revision(revision)
            .build();
        let mut blocked = PhotonContact::from_contact(&blocked);
        let opaque = blocked.to_opaque_document(&self.user_identity)?;
        let mutation = ContactMutation::add(revision, opaque);
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(confirmed) = result else {
            return Err(ArgumentError::new("Unexpected response type").into());
        };

        blocked.set_revision(confirmed);
        self.contacts_revision.set(confirmed);
        self.put_contact(&blocked).await?;
        self.blocked_contacts.lock().unwrap().insert(*user_id);
        Ok(Box::new(blocked) as Contact)
    }

    fn remove_contacts_cache(&self, revision: i32, contact_ids: &[Id]) {
        if let Err(error) = self.repository
            .borrow_mut()
            .remove_contacts(revision, contact_ids)
        {
            warn!("Failed to remove contacts from repository: {error}");
        }
        for contact_id in contact_ids {
            self.repository.borrow_mut().remove_channel(self.contacts_revision.get(), contact_id).ok();
            self.session_contexts.lock().unwrap().remove(contact_id);
            self.blocked_contacts.lock().unwrap().remove(contact_id);
        }
        self.contacts_revision.set(revision);
        self.contact_listener.on_contacts_removed(contact_ids);
    }

    fn clear_contacts_cache(&self) {
        let revision = self.contacts_revision.get();
        if let Err(error) = self.repository.borrow_mut().clear_contacts(revision) {
            warn!("Failed to clear contacts from repository: {error}");
        }
        self.session_contexts.lock().unwrap().clear();
        self.blocked_contacts.lock().unwrap().clear();
        self.contact_listener.on_contacts_cleared();
    }

    pub(crate) async fn create_channel(&self, nci: NewChannelInfo) -> Result<PhotonChannel> {
        let session_key = signature::KeyPair::random();
        let session_id  = Id::from(session_key.public_key());
        let encrypted_sk  = self.self_context.borrow_mut().encrypt_into(
                session_key.private_key().as_bytes()
            ).map_err(|e|
                EncodingError::new(format!("Failed to encrypt session key: {}", e))
            )?;

        let revised = nci.revised(session_id, encrypted_sk);
        let request = RPCRequest::create_channel(0, revised);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ChannelCreate(ci) = result else {
            return Err(EncodingError::new("Rpc result is not channel info").into());
        };

        // The session key is self-encrypted, same as the one just sent to the server.
        let mut channel = PhotonChannel::from(&ci, ci.session_key().map(|key| key.to_vec()));

        // Register the session so the new channel can send/receive right away.
        let session_identity = CryptoIdentity::from(session_key);
        self.session_contexts.lock().unwrap().insert(
            *channel.id(),
            self.session_context(*channel.id(), session_identity, true),
        );

        // 2. Save the channel (and its members, if any) locally first for a responsive UI.
        self.put_channel(&channel)?;

        // 3. Add the channel as a new contact (sync cross all devices)
        let opaque_data = channel.serialize();
        let opaque_contact = channel.to_opaque(opaque_data);
        let mutation = ContactMutation::add(self.contacts_revision.get(), opaque_contact);
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = result else {
            return Err(EncodingError::new(
                "RPC result is not contact mutate response").into());
        };

        channel.set_revision(revision);
        self.contacts_revision.set(revision);
        self.put_channel(&channel)?;
        Ok(channel)
    }

    pub(crate) async fn delete_channel(&self, channel_id: &Id) -> Result<()> {
        let Some(channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(format!(
                "Channel {channel_id} does not exist")).into());
        };
        if channel.owner() != self.user_id() {
            return Err(StateError::new("Only the owner can delete the channel"));
        }

        let request = RPCRequest::delete_channel(0);
        let result = self.send_rpccall(channel_id, request).await?;
        let response::Value::ChannelDelete(_) = &result else {
            return Err(EncodingError::new(
                "The response is not for delete channel RPC").into());
        };

        let mutation = ContactMutation::remove(
            self.contacts_revision.get(),
            vec![*channel_id]
        );
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = result else {
            return Err(EncodingError::new(
                "RPC result is not contact mutate response").into());
        };
        if revision < self.contacts_revision.get() {
            return Err(StateError::new(
                "Contact removal revision is outdated").into());
        }
        self.contacts_revision.set(revision);
        self.remove_contact(channel_id).await;
        self.contact_listener.on_contacts_removed(&[*channel_id]);
        Ok(())
    }

    pub(crate) async fn join_channel(&self, ticket: InviteTicket) -> Result<PhotonChannel> {
        assert!(
            ticket.is_genuine(),
            "Invite ticket is not genuine"
        );
        assert!(
            !ticket.is_expired(),
            "Invite ticket is expired"
        );
        assert!(
            ticket.invitee().is_some_and(|id| id == self.user_id()),
            "Invite ticket is not for this user, only invitee can join the channel"
        );

        // check the session key is valid and re-entry with self-encryption context.
        let session_sk = if ticket.is_named_ticket() {
            self.user_identity.decrypt_into(
                ticket.inviter(),
                ticket.session_key()
            ).map_err(|_| ArgumentError::new("Invalid session key"))?
        } else {
            ticket.session_key().to_vec()
        };
        let session_key = signature::KeyPair::try_from(session_sk.as_slice())
            .map_err(|_| ArgumentError::new("Invalid session key"))?;

        let session_id = Id::from(session_key.public_key());
        if ticket.session_id() != &session_id {
            return Err(ArgumentError::new(
                "Invitation session key does not match session ID"));
        }
        self.self_context.borrow_mut().reset_nonce();
        let encrypted_sk = self.self_context.borrow_mut().encrypt_into(
            &session_sk
        ).map_err(|e|
            EncodingError::new(format!("Failed to encrypt session key: {}", e))
        )?;

        let revised = ticket.revise(encrypted_sk);

        // Check whether it has already joined the channel or not.
        if let Some(channel) = self.get_channel(ticket.channel_id()) {
            if channel.has_session_key() && channel.has_member(self.user_id()) {
                return Err(StateError::new("Already joined the channel").into());
            }
        }

        // Request to join channel.
        let request = RPCRequest::join_channel(0, revised);

        let result = match self.send_rpccall(ticket.channel_id(), request).await {
            Ok(result) => result,
            Err(error) => {
                // The join may still have succeeded even when the RPC response is
                // lost; the channel-join notification carries the local state.
                if let Some(channel) = self.get_channel(ticket.channel_id()) {
                    if channel.has_member(self.user_id()) {
                        let session_identity = CryptoIdentity::from(session_key);
                        self.session_contexts.lock().unwrap().insert(
                            *channel.id(),
                            self.session_context(*channel.id(), session_identity, true),
                        );
                        return Ok(channel);
                    }
                }
                return Err(error.into());
            }
        };
        let response::Value::ChannelJoin(ci) = result else {
            return Err(EncodingError::new("RPC result is not channel info").into());
        };
        if ci.session_key().is_none() {
            return Err(EncodingError::new(
                "Invalid channel info: session key is missing").into());
        }
        if ci.channel_id() != ticket.channel_id() {
            return Err(EncodingError::new(
                "Channel ID in response does not match invitation"
            ).into());
        }

        let mut channel = PhotonChannel::from(
            &ci,
            ci.session_key().map(|v|v.to_vec())
        );

        // Register the session so the joined channel can send/receive right away.
        let session_identity = CryptoIdentity::from(session_key);
        self.session_contexts.lock().unwrap().insert(
            *channel.id(),
            self.session_context(*channel.id(), session_identity, true),
        );

        // Save the channel (and its members, if any) locally first for a responsive UI.
        self.put_channel(&channel)?;

        // Add the channel as a new contact (to sync cross all devices)
        let opaque_data = channel.serialize();
        let opaque_contact = channel.to_opaque(opaque_data);
        let mutation = ContactMutation::add(self.contacts_revision.get(), opaque_contact);
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = result else {
            return Err(EncodingError::new(
                "RPC result is not contact mutate response").into());
        };

        channel.set_revision(revision);
        self.contacts_revision.set(revision);
        self.put_channel(&channel)?;

        // self.channel_listener.on_joined_channel(&channel);
        Ok(channel)
    }

    pub(crate) async fn leave_channel(&self, channel_id: &Id) -> Result<()> {
        let Some(channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(
                &format!("Channel {} is not existing", channel_id)));
        };
        if channel.owner() == self.user_id() {
            return Err(StateError::new("The owner cannot leave the channel"));
        }

        let request = RPCRequest::leave_channel(0);
        let result = self.send_rpccall(channel_id, request).await?;
        let response::Value::ChannelLeave(_) = result else {
            return Err(EncodingError::new(
                "RPC result is not channel leave response").into());
        };

        let mutation = ContactMutation::remove(
            self.contacts_revision.get(),
            vec![*channel_id]
        );
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = result else {
            return Err(EncodingError::new(
                "RPC result is not contact mutate response").into());
        };
        if revision < self.contacts_revision.get() {
            return Err(StateError::new(
                "Contact removal revision is outdated").into());
        }
        self.contacts_revision.set(revision);
        self.remove_contact(channel_id).await;
        self.contact_listener.on_contacts_removed(&[*channel_id]);

        //self.channel_listener.on_left_channel(&channel);
        Ok(())
    }

    pub(crate) async fn create_invite_ticket(
        &self,
        channel_id: &Id,
        invitee: Option<Id>,
    ) -> Result<InviteTicket> {
        let Some(channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(&format!(
                "Channel {} does not exist", channel_id)));
        };
        let Some(om) = channel.member(self.user_id()) else {
            return Err(StateError::new("Not a member of the channel"));
        };

        // check member and permission;
        if let Some(ref invitee) = invitee {
            if channel.has_member(invitee) {
                return Err(ArgumentError::new("The invitee is already a channel member").into());
            }
        }
        match channel.permission() {
            Permission::Public | Permission::MemberInvite => {
                if om.is_banned() {
                    return Err(StateError::new("You are banned from the channel").into());
                }
            }
            Permission::ModeratorInvite => {
                if !om.is_moderator() && !om.is_owner() {
                    return Err(StateError::new("You are not a moderator of the channel").into());
                }
            }
            Permission::OwnerInvite => {
                if !om.is_owner() {
                    return Err(StateError::new("Insufficient permission: only the channel owner can invite").into());
                }
            }
        }
        self.self_context.borrow_mut().reset_nonce();
        let session_sk = self.self_context.borrow_mut().decrypt_into(
            channel.session_key()
        )?;
        let session_key = signature::KeyPair::try_from(session_sk.as_slice())
            .map_err(|e| {
                EncodingError::new(&format!("Failed to create session key: {}", e))
            })?;
        let session_id = Id::from(session_key.public_key());
        let session_sk = if let Some(ref invitee) = invitee {
            self.user_identity.encrypt_into(invitee, session_sk.as_slice())?
        } else {
            session_sk
        };

        InviteTicket::create(
            &self.user_identity,
            *channel_id,
            session_id,
            invitee,
            boson_api::as_ms!(SystemTime::now() + Duration::from_millis(InviteTicket::DEFAULT_EXPIRATION_MS)) as i64,
            session_sk,
        ).map_err(|e| e.into())
    }

    pub(crate) async fn transfer_channel_ownership(
        &self,
        channel_id: &Id,
        new_owner: Id,
    ) -> Result<()> {
        let Some(mut channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(
                &format!("Channel {channel_id} is not existing")));
        };
        if channel.owner() != self.user_id() {
            return Err(StateError::new("Only the channel owner can transfer ownership"));
        }
        let _ = channel.try_load_members()?;

        let Some(target) = channel.member(&new_owner) else {
            return Err(StateError::new("New owner is not a member of the channel"));
        };
        if target.is_banned() {
            return Err(StateError::new("New owner is banned from the channel"));
        }
        if &new_owner == channel.owner() {
            return Err(ArgumentError::new("The new owner is already the channel owner").into());
        }

        let request = RPCRequest::transfer_channel_ownership(0, new_owner);
        let result = self.send_rpccall(channel_id, request).await?;
        let response::Value::ChannelOwnershipTransfer(_) = result else {
            return Err(StateError::new("Unexpected response for channel ownership transfer"));
        };

        let updated = channel.edit_channel()
            .set_owner(new_owner)
            .build();

        let _ = self.repository
            .borrow_mut()
            .put_channel_locally(&updated);
        // TODO: invalidate contact cache or channel cache;
        Ok(())
    }

    pub(crate) async fn rotate_channel_session_key(
        &self,
        channel_id: Id,
        session_key: Option<Vec<u8>>,
    ) -> Result<()> {
        // TODO: DO NOT DELETE until the function is fully refinemented.
        let Some(channel) = self.get_channel(&channel_id) else {
            return Err(StateError::new("Channel not found").into());
        };
        if channel.owner() != self.user_id() {
            return Err(StateError::new("Only the channel owner can rotate the session key"));
        }

        let keypair = match session_key {
            Some(bytes) => signature::KeyPair::try_from(bytes.as_slice())
                .map_err(|_| ArgumentError::new("Invalid session key"))?,
            None => signature::KeyPair::random(),
        };
        let encrypted_key = {
            let mut sessions = self.session_contexts.lock().unwrap();
            let current_session = sessions
                .get_mut(&channel_id)
                .ok_or_else(|| StateError::new("Channel session key is missing"))?;
            current_session
                .tx_crypto_context()?
                .encrypt_into(keypair.private_key().as_ref())?
        };

        let session_id = Id::from(keypair.public_key());
        let rotation = ChannelSessionKeyRotation::new(session_id, encrypted_key);
        let request = RPCRequest::rotate_channel_session_key(0, rotation);

        let result = self.send_rpccall(&channel_id, request).await?;
        let _ = result;

        self.self_context.borrow_mut().reset_nonce();
        let stored_key = self.self_context.borrow_mut().encrypt_into(
            keypair.private_key().as_ref()
        ).map_err(|e| {
            EncodingError::new(format!("Failed to encrypt session key: {}", e))
        })?;
        let updated = channel.edit_channel()
            .set_session_key(stored_key)
            .build();
        self.session_contexts.lock().unwrap().insert(
            channel_id,
            self.session_context(channel_id, CryptoIdentity::from(keypair), true),
        );
        self.put_channel(&updated)?;
        Ok(())
    }

    pub(crate) async fn update_channel_info(&self, channel: PhotonChannel) -> Result<()> {
        let Some(origin) = self.get_channel(channel.id()) else {
            return Err(StateError::new(&format!("Channel {} not found", channel.id())).into());
        };
        if channel.owner() != self.user_id() {
            return Err(StateError::new("Only the channel owner can update channel information"));
        }

        if origin == channel {
            return Err(ArgumentError::new("Channel information has not changed").into());
        }

        let mut changes = ChannelInfoUpdate::new();
        if origin.permission() != channel.permission() {
            changes = changes.set_permission(channel.permission());
        }
        if origin.name() != channel.name() {
            changes = changes.set_name(channel.name().map(ToString::to_string));
        }
        if origin.notice().as_deref() != channel.notice() {
            changes = changes.set_notice(channel.notice().map(ToString::to_string));
        }
        if origin.is_announced() != channel.is_announced() {
            changes = changes.set_announce(channel.is_announced());
        }

        let request = RPCRequest::update_channel_info(0, changes);
        let result  = self.send_rpccall(channel.id(), request).await?;
        let response::Value::ChannelInfoUpdate(_) = &result else {
            return Err(StateError::new(
                "Unexpected response for update_channel_info"));
        };

        // TODO: invalidate contact cache;
        let _ = self.repository
            .borrow_mut()
            .put_channel_locally(&channel);
        Ok(())
    }

    pub(crate) async fn set_channel_members_role(
        &self,
        channel_id: &Id,
        members: Vec<Id>,
        role: Role,
    ) -> Result<()> {
        assert!(
            !members.is_empty(),
            "Members list cannot be empty"
        );

        assert!(
            role != Role::Owner && role != Role::Banned,
            "Cannot set role to owner or banned"
        );

        let Some(mut channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(&format!(
                "Channel {} is not existing", channel_id)));
        };

        let _ = channel.try_load_members()?; // TODO: loading on-demanding.

        let Some(member) = channel.member(self.user_id()) else {
            return Err(StateError::new("Not a member of the channel").into());
        };
        if !member.is_owner() && !member.is_moderator() {
            return Err(StateError::new(
                "Only a channel owner or moderator",
            ).into());
        }

        let req = RPCRequest::update_channel_members_role(0, members.to_vec(), role);
        let rsp = self.send_rpccall(channel_id, req).await?;
        let RValue::ChannelMembersRoleUpdate(ids) = &rsp else {
            return Err(StateError::new(
                "Unexpected response for update_channel_members_role"));
        };

        channel.invalidate_members();
        self.repository
            .borrow_mut()
            .update_channel_members_role(channel_id, ids, role as i32)?;

        Ok(())
    }

    pub(crate) async fn ban_channel_members(
        &self,
        channel_id: &Id,
        members: Vec<Id>
    ) -> Result<()> {
        assert!(
            !members.is_empty(),
            "Members list cannot be empty"
        );

        let Some(mut channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(&format!(
                "Channel {} is not existing", channel_id)));
        };
        let Some(member) = channel.member(self.user_id()) else {
            return Err(StateError::new("Not a member of the channel").into());
        };
        if !member.is_owner() && !member.is_moderator() {
            return Err(StateError::new(
                "Only a channel owner or moderator").into());
        }

        let request = RPCRequest::ban_channel_members(0, members);
        let result  = self.send_rpccall(channel_id, request).await?;
        let response::Value::ChannelMembersBan(ids) = &result else {
            return Err(StateError::new(
                "Unexpected response for ban_channel_members").into());
        };

        if !ids.is_empty() {
            channel.invalidate_members();
            self.repository
                .borrow_mut()
                .update_channel_members_role(channel_id, ids, Role::Banned as i32)?;
        }
        Ok(())
    }

    pub(crate) async fn unban_channel_members(
        &self,
        channel_id: &Id,
        members: Vec<Id>,
    ) -> Result<()> {
        assert!(
            !members.is_empty(),
            "Members list cannot be empty"
        );

        let Some(mut channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(&format!(
                "Channel {} is not existing", channel_id)));
        };
        let Some(member) = channel.member(self.user_id()) else {
            return Err(StateError::new("Not a member of the channel").into());
        };
        if !member.is_owner() && !member.is_moderator() {
            return Err(StateError::new(
                "Only a channel owner or moderator").into());
        }

        let request = RPCRequest::unban_channel_members(0, members);
        let result = self.send_rpccall(channel_id, request).await?;
        let response::Value::ChannelMembersUnban(ids) = &result else {
            return Err(StateError::new(
                "Unexpected response for unban_channel_members").into());
        };

        if !ids.is_empty() {
            channel.invalidate_members();
            self.repository
                .borrow_mut()
                .update_channel_members_role(channel_id, ids, Role::Member as i32)?;
        }
        Ok(())
    }

    pub(crate) async fn remove_channel_members(
        &self,
        channel_id: &Id,
        members: Vec<Id>,
    ) -> Result<()> {
        assert!(
            !members.is_empty(),
            "Members list cannot be empty"
        );

        let Some(mut channel) = self.get_channel(channel_id) else {
            return Err(StateError::new(&format!(
                "Channel {} is not existing", channel_id)));
        };
        let Some(member) = channel.member(self.user_id()) else {
            return Err(StateError::new("Not a member of the channel").into());
        };
        if !member.is_owner() && !member.is_moderator() {
            return Err(StateError::new(
                "Only a channel owner or moderator").into());
        }

        let request = RPCRequest::remove_channel_members(0, members);
        let rsp = self.send_rpccall(channel_id, request).await?;
        let response::Value::ChannelMembersRemove(ids) = &rsp else {
            return Err(StateError::new(
                "Unexpected response for remove_channel_members").into());
        };

        if !ids.is_empty() {
            channel.invalidate_members();
            self.repository
                .borrow_mut()
                .remove_channel_members(channel_id, ids)?;
        }
        Ok(())
    }

    pub(crate) async fn get_contact(&self, id: &Id) -> Option<PhotonContact> {
        self.repository.borrow_mut().contact(id).ok().flatten()
    }

    pub(crate) async fn get_contacts(&self) -> Vec<PhotonContact> {
        self.repository
            .borrow_mut()
            .contacts()
            .ok()
            .unwrap_or_default()
    }

    pub(crate) async fn update_contact(&self, contact: PhotonContact) -> Result<()> {
        let contact_id = contact.id();
        let Some(origin) = self.get_contact(contact_id).await else {
            return Err(StateError::new(&format!("Contact {} is not existing", contact_id)));
        };
        if origin.remark() == contact.remark()
            && origin.tags() == contact.tags()
            && origin.is_muted() == contact.is_muted()
            && origin.is_blocked() == contact.is_blocked()
        {
            return Err(StateError::new("No changes detected"));
        }

        if contact.revision() < origin.revision() {
            return Err(StateError::new(
                "Updated contact version is older than the current version",
            ));
        }

        let updated = origin.edit()
            .set_remark(contact.remark().map(ToString::to_string))
            .set_tags(contact.tags().map(ToString::to_string))
            .set_muted(contact.is_muted())
            .set_blocked(contact.is_blocked())
            .set_revision(origin.revision().saturating_add(1))
            .build();
        let mut updated = PhotonContact::from_contact(&updated);

        let mutation = ContactMutation::update(
            self.contacts_revision.get(),
            updated.to_opaque_document(&self.user_identity)?
        );
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = result else {
            return Err(StateError::new("Unexpected response for contact mutate").into());
        };

        let local_revision = revision.max(origin.revision().saturating_add(1));
        updated.set_revision(local_revision);
        self.contacts_revision.set(revision);
        if updated.is_blocked() {
            self.blocked_contacts.lock().unwrap().insert(*updated.id());
        } else {
            self.blocked_contacts.lock().unwrap().remove(updated.id());
        }
        self.put_contact(&updated).await?;
        let updated_contact: Contact = Box::new(updated);
        self.contact_listener.on_contacts_updated(&[updated_contact]);

        Ok(())
    }

    pub(crate) async fn remove_contacts(&self, contact_ids: &[Id]) -> Result<()> {
        assert!(
            !contact_ids.is_empty(),
            "No contact IDs specified"
        );

        let mutation = ContactMutation::remove(
            self.contacts_revision.get(),
            contact_ids.to_vec()
        );
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(revision) = result else {
            return Err(EncodingError::new("RPC result is not a contact revision").into());
        };
        if revision < self.contacts_revision.get() {
            return Err(StateError::new("Contact removal revision is outdated"));
        }

        self.remove_contacts_cache(revision, contact_ids);
        // TODO: adavance revision
        Ok(())
    }

    pub(crate) async fn clear_contacts(&self) -> Result<()> {
        let revision = self.contacts_revision.get();
        let mutation = ContactMutation::clear(revision);
        let request = RPCRequest::contact_mutate(0, mutation);

        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactMutate(confirmed) = result else {
            return Err(EncodingError::new("RPC result is not a contact revision").into());
        };

        if confirmed < revision || confirmed < self.contacts_revision.get() {
            return Err(StateError::new("Contact clear revision is outdated"));
        }

        self.contacts_revision.set(confirmed);
        self.clear_contacts_cache();
        Ok(())
    }

    pub(crate) fn message(&self, recipient: Option<Id>) -> Box<dyn MessageBuilderHandler> {
        Box::new(SessionMessageBuilder {
            recipient,
            client: self.client_handle(),
            user_identity: self.user_identity.clone(),
            device_identity: self.device_identity.clone(),
            friend_sessions: self.session_contexts.clone(),
            blocked_contacts: self.blocked_contacts.clone(),
            origin_timestamp: self.origin_timestamp() as i64,
            conversation_contact: recipient.and_then(|id| {
                self.repository
                    .borrow_mut()
                    .contact(&id)
                    .ok()
                    .flatten()
                    .map(|contact| Box::new(contact) as Contact)
            }),
            conversations: self.repository.borrow().conversations_handle(),
            peer_id: *self.peer_id(),
            content_type: None,
            content_disposition: None,
            headers: Vec::new(),
            body: None,
            text: false,
        })
    }

}

/// Collects a message on this side, then publishes it.
///
/// The session itself stays on the verticle thread, so the builder captures the
/// pieces it needs (identities, the MQTT handle, the conversation cache) and
/// does all encoding and encryption on the caller's thread.
pub(crate) struct SessionMessageBuilder {
    recipient: Option<Id>,
    client: ClientHandle,
    user_identity: CryptoIdentity,
    device_identity: CryptoIdentity,
    friend_sessions: Arc<std::sync::Mutex<HashMap<Id, SessionContext>>>,
    blocked_contacts: Arc<std::sync::Mutex<HashSet<Id>>>,
    origin_timestamp: i64,
    conversation_contact: Option<Contact>,
    conversations: Arc<Mutex<HashMap<Id, PhotonConversation>>>,
    peer_id: Id,
    content_type: Option<String>,
    content_disposition: Option<ContentDisposition>,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    text: bool,
}

impl MessageBuilderHandler for SessionMessageBuilder {
    fn with_content_type(mut self: Box<Self>, content_type: &str) -> Box<dyn MessageBuilderHandler> {
        self.content_type = Some(content_type.to_string());
        self
    }

    fn with_content_disposition(mut self: Box<Self>, disposition: ContentDisposition) -> Box<dyn MessageBuilderHandler> {
        self.content_disposition = Some(disposition);
        self
    }

    fn with_text_body(mut self: Box<Self>, text: &str) -> Box<dyn MessageBuilderHandler> {
        self.body = Some(text.as_bytes().to_vec());
        self.text = true;
        self
    }

    fn with_binary_body(mut self: Box<Self>, data: Vec<u8>) -> Box<dyn MessageBuilderHandler> {
        self.body = Some(data);
        self.text = false;
        self
    }

    fn with_header(mut self: Box<Self>, key: &str, value: &str) -> Box<dyn MessageBuilderHandler> {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    fn send(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<Message>> + Send>> {
        Box::pin(async move {
            let this = *self;
            let recipient = this
                .recipient
                .ok_or_else(|| ArgumentError::new("Message recipient is required"))?;
            if this.blocked_contacts.lock().unwrap().contains(&recipient) {
                return Err(StateError::new("Cannot send a message to a blocked contact").into());
            }
            let mqtt: mqtt::Client = this
                .client
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| StateError::new("Messaging client is not running"))?;
            let body = this
                .body
                .ok_or_else(|| ArgumentError::new("Message content is required"))?;
            let headers = build_message_headers(
                this.headers,
                this.content_type,
                this.content_disposition,
                this.text,
            );

            let timestamp = this.origin_timestamp;
            let content = PhotonContent {
                headers: headers_to_cbor(&headers)?,
                format: if this.text {
                    ContentFormat::Text
                } else {
                    ContentFormat::Binary
                },
                body: if this.text {
                    Value::Text(
                        String::from_utf8(body)
                            .map_err(|_| EncodingError::new("Text message is not UTF-8"))?,
                    )
                } else {
                    Value::Bytes(body)
                },
            };
            let content_bytes = content.serialize()?;
            let (encrypted, session_context) = {
                let mut sessions = this.friend_sessions.lock().unwrap();
                let session = sessions
                    .get_mut(&recipient)
                    .ok_or_else(|| StateError::new(format!("No friend session for {recipient}")))?;
                (session.tx_crypto_context()?.encrypt_into(&content_bytes)?, session.clone())
            };
            let (mut wire, payload) = super::transport::seal_for(
                &this.user_identity,
                &this.device_identity,
                &this.peer_id,
                &recipient,
                MessageType::Content as i32,
                timestamp,
                encrypted,
                None,
                "content",
            )?;
            super::transport::publish_outbox(&mqtt, payload, "message").await?;

            // The returned message mirrors what a recipient would see, so the
            // caller can render it without a round trip.
            wire.set_from(*this.user_identity.id());
            wire.set_sent(boson_api::as_ms!(SystemTime::now()) as i64);
            wire.payload_bytes = content_bytes;
            wire.content = Some(content);
            wire.set_converstation_id(recipient);
            let contact = this
                .conversation_contact
                .unwrap_or_else(|| Box::new(PhotonContact::new(recipient, ContactType::Auto, timestamp)) as Contact);
            let boxed_message: Message = Box::new(wire);
            update_conversation(&this.conversations, contact, session_context, boxed_message.clone())?;
            Ok(boxed_message)
        })
    }
}

/// Folds the typed builder fields into the header map the wire format takes.
///
/// A non-text body defaults to `application/octet-stream`; an explicit content
/// type always wins, and so does an explicit content disposition.
fn build_message_headers(
    headers: Vec<(String, String)>,
    declared_type: Option<String>,
    disposition: Option<ContentDisposition>,
    text: bool,
) -> HashMap<String, serde_json::Value> {
    let mut merged: HashMap<String, serde_json::Value> = headers
        .into_iter()
        .map(|(key, value)| (key, serde_json::Value::String(value)))
        .collect();

    if let Some(declared_type) = declared_type {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(declared_type),
        );
    } else if !text && !merged.contains_key(content_type::HEADER_NAME) {
        merged.insert(
            content_type::HEADER_NAME.into(),
            serde_json::Value::String(content_type::ContentType::ApplicationOctetStream.as_str().into()),
        );
    }

    if let Some(disposition) = disposition {
        merged.insert(
            ContentDisposition::HEADER_NAME.into(),
            serde_json::Value::String(disposition.value()),
        );
    }
    merged
}

/// Converts JSON header values into their CBOR representation.
fn headers_to_cbor(headers: &HashMap<String, serde_json::Value>) -> Result<HashMap<String, Value>> {
    headers
        .iter()
        .map(|(key, value)| {
            serde_cbor::value::to_value(value)
                .map(|value| (key.clone(), value))
                .map_err(|error| {
                    EncodingError::new(format!("Encoding header failed: {error}")).into()
                })
        })
        .collect()
}

/// Folds a message into its conversation, creating the conversation on demand.
pub(super) fn update_conversation(
    conversations: &Arc<Mutex<HashMap<Id, PhotonConversation>>>,
    contact: Contact,
    session_context: SessionContext,
    message: Message,
) -> Result<()> {
    let mut conversations = conversations.lock().unwrap();
    let contact_id = *contact.id();
    let contact = PhotonContact::from_contact(&contact);
    let conversation = conversations
        .entry(contact_id)
        .or_insert_with(|| PhotonConversation::new(contact, session_context));
    conversation.update(message)
}

impl RpcServer {
    async fn send_message_internal(&self, mut message: PhotonMessage) -> Result<()> {
        use crate::MessageHandler;

        let payload = message.payload_as_bytes();
        let encrypted = if payload.is_empty() {
            Vec::new()
        } else {
            match message.message_type() {
                MessageType::Content => {
                    let conversation_id = message
                        .conversation_id()
                        .copied()
                        .unwrap_or_else(|| *message.recipient());
                    let mut sessions = self.session_contexts.lock().unwrap();
                    let session = sessions.get_mut(&conversation_id).ok_or_else(|| {
                        StateError::new(format!(
                            "No channel session context for {conversation_id}"
                        ))
                    })?;
                    session.tx_crypto_context()?.encrypt_into(payload)?
                }
                MessageType::Control => {
                    if message.recipient() == self.peer_id() {
                        payload.to_vec()
                    } else {
                        self.user_identity
                            .encrypt_into(message.recipient(), payload)
                            .map_err(|error| {
                                EncodingError::new(format!(
                                    "Encrypting message payload failed: {error}"
                                ))
                            })?
                    }
                }
                MessageType::State => {
                    return Err(StateError::new(
                        "INTERNAL ERROR: messaging client is not supposed to send state messages",
                    )
                    .into());
                }
                MessageType::Handshake => self
                    .user_identity
                    .encrypt_into(message.recipient(), payload)
                    .map_err(|error| {
                        EncodingError::new(format!(
                            "Encrypting message payload failed: {error}"
                        ))
                    })?,
            }
        };

        message.set_payload(encrypted);
        let encoded = message.serialize();
        let envelope = self
            .device_identity
            .encrypt_into(self.peer_id(), &encoded)
            .map_err(|error| {
                AuthenticationError::new(format!("Encrypting message failed: {error}"))
            })?;

        let mqtt: mqtt::Client = self
            .mqtt_client
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| StateError::new("Messaging client is not running"))?;
        super::transport::publish_outbox(&mqtt, envelope, "message").await
    }

    pub(super)async fn send_rpccall(
        &self,
        recipient: &Id,
        request: RPCRequest,
    ) -> Result<response::Value> {
        let timestamp = self.origin_timestamp();
        let method = request.method();
        let request = request.with_id(timestamp);
        let payload = request.serialize()?;
        let call = self
            .pending_calls
            .register(timestamp, method, *recipient)?;

        let message_id = crate::interfaces::generate_id(self.device_identity.id(), timestamp);
        let message = PhotonMessage::outgoing(
            message_id,
            *recipient,
            MessageType::Control,
            timestamp,
            Value::Bytes(payload),
        );

        tokio::time::timeout(RPC_TIMEOUT, async {
            self.send_message_internal(message).await?;
            call.await
        })
        .await
        .map_err(|_| {
            warn!("RPC '{method:?}' timed out");
            RPCError::timeout()
        })?
    }
}
