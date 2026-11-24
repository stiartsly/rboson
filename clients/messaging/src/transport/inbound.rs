use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};
use serde::Deserialize;
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use log::{debug, error, info, warn, trace};

use boson_api::{
    Result,
    errors::{ArgumentError, StateError},
    signature,
    CryptoIdentity,
    Id,
    Identity,
};

use crate::{
    ChannelHandler, ChannelMember, ChannelMemberHandler, ConnectionListener, FriendRequest, FriendRequestHandler, Message, MessageOps, MessageOriginated, channel::{Channel, Permission, Role}, contact::{Contact, ContactHandler, ContactType}, errors::{
        AuthenticationError,
        EncodingError,
    }, message::MessageType
};
use crate::internal::{
    ContactMutation,
    ContactSync,
    Handshake,
    PhotonFriendRequest,
    HandshakeType,
    PhotonContent,
    MutationData,
    OpaqueContact,
    PhotonChannel,
    PhotonContact,
    PhotonMessage,
    PhotonConversation,
    SessionContext,
    SyncType,
    Topic,
    notification::{Body, Notification}
};
use crate::rpc::request::RPCRequest;
use crate::rpc::response;
use super::RpcServer;

#[derive(Deserialize)]
pub(super) struct ChannelContactInfo {
    #[serde(rename = "o")]
    pub(super) owner: Id,
    #[serde(rename = "p")]
    pub(super) permission: i32,
    #[serde(rename = "nt", default)]
    pub(super) notice: Option<String>,
    #[serde(rename = "a", default)]
    pub(super) announced: bool,
}

pub(super) struct DecodedContact {
    pub(super) contact: PhotonContact,
    pub(super) session: Option<CryptoIdentity>,
    pub(super) channel: Option<PhotonChannel>,
}

pub(super) struct ContactState {
    pub(super) contacts: HashMap<Id, Contact>,
    pub(super) sessions: HashMap<Id, SessionContext>,
    pub(super) channels: HashMap<Id, PhotonChannel>,
    pub(super) revision: i32,
}

impl PhotonMessage {
    fn set_content(&mut self, content: PhotonContent) -> Message {
        self.content = Some(content);
        Box::new(self.clone()) as Message
    }
}

impl RpcServer {
    pub(super) async fn dispatch(
        &self,
        topic: Topic,
        mut message: PhotonMessage
    ) {
        message.set_received();

        let Some(from) = message.from().copied() else {
            warn!("Incoming message has no sender, ignoring it");
            return;
        };
        let recipient = message.recipient();
        let msg_type  = message.message_type();

        match topic {
            Topic::UserInbox => {
                if msg_type == MessageType::Control {
                    warn!("Received {msg_type} message from user inbox, ignoring it");
                    return;
                }
                let conv_id = if recipient == self.user_id() {
                    from
                } else {
                    *recipient
                };
                message.set_converstation_id(conv_id);
            }
            Topic::UserOutbox => {
                if msg_type == MessageType::Control {
                    warn!("Received {msg_type} message from user outbox, ignoring it");
                    return;
                }
                message.set_converstation_id(*recipient);
            }
            Topic::DeviceInbox => {
                if msg_type != MessageType::Control {
                    warn!("Received {msg_type} message from device inbox, ignoring it");
                    return;
                }
                message.set_converstation_id(*recipient);
            }
            _ => {
                warn!("Received {msg_type} message from unsupported topic, ignoring it");
                return;
            }
        }

        if let Err(error) = self.check_and_decrypt_message(&mut message).await {
            warn!("{error}");
            return;
        }

        let _ = match topic {
            Topic::UserInbox => self.process_inbox_message(message, &from).await,
            Topic::DeviceInbox => { self.on_device_inbox_message(message, &from); Ok(()) },
            Topic::UserOutbox => self.process_outbox_message(message, &from).await,
            Topic::DeviceOutbox => {
                warn!("Ignoring message from device outbox");
                return;
            }
        }.map_err(|error| warn!("{error}"));
    }

    async fn check_and_decrypt_message(&self, message: &mut PhotonMessage) -> Result<()> {
        let from = *message.from_or_throw()?;
        let recipient = *message.recipient();
        let message_type = message.message_type();
        let payload = match std::mem::replace(&mut message.payload, Value::Null) {
            Value::Bytes(payload) => payload,
            other => {
                message.payload = other;
                return Err(EncodingError::new("Message payload is not binary").into());
            }
        };

        let decrypted = match message_type {
            MessageType::Content => {
                let Some(conversation_id) = message.conversation_id() else {
                    return Err(StateError::new(
                        "Received a message with no conversation Id")
                    .into());
                };
                let Some(contact) = self.get_contact(&conversation_id).await else {
                    return Err(StateError::new(format!(
                        "Received a message for unknown contact {conversation_id}"
                    )).into());
                };

                if contact.is_blocked() {
                    return Err(StateError::new(
                        "Received a message from a blocked contact"
                    ).into());
                }
                if contact.contact_type() == ContactType::Auto {
                    return Err(StateError::new(
                        "Received a message from a contact of no friend."
                    ).into());
                }

                // A blocked user's messages in a channel are still delivered: the channel is a shared
				// conversation, and dropping one member's messages would leave holes (replies without
				// what they answer) and a view that differs from the other members'. The channel's own
				// crypto context (looked up below via `conversation_id`, the channel id) already handles
				// per-sender decryption through `rx_crypto_context_for`.


                let mut sessions = self.session_contexts.lock().unwrap();
                let session = sessions.get_mut(&conversation_id).ok_or_else(|| {
                    StateError::new(format!("No session context for {conversation_id}"))
                })?;
                if from == *self.user_id() {
                    session.tx_crypto_context()?.decrypt_into(&payload)?
                } else {
                    session.rx_crypto_context_for(&from)?.decrypt_into(&payload)?
                }
            }
            MessageType::Control | MessageType::State => {
                if from == *self.peer_id() {
                    payload
                } else if recipient == *self.user_id() {
                    self.user_identity.decrypt_into(&from, &payload).map_err(|error| {
                        AuthenticationError::new(format!("Decrypting {message_type:?} failed: {error}"))
                    })?
                } else {
                    let channel = self.get_channel(&recipient).ok_or_else(|| {
                        StateError::new(format!("Received a {message_type:?} from unknown channel {recipient}"))
                    })?;
                    if channel.is_blocked() {
                        return Err(StateError::new("Received a message from a blocked channel").into());
                    }
                    let mut sessions = self.session_contexts.lock().unwrap();
                    let session = sessions.get_mut(&recipient).ok_or_else(|| {
                        StateError::new(format!("No channel session context for {recipient}"))
                    })?;
                    session.rx_crypto_context()?.decrypt_into(&payload)?
                }
            }
            MessageType::Handshake => {
                let friend_id = if from == *self.user_id() { &recipient } else { &from };
                self.user_identity.decrypt_into(&friend_id, &payload).map_err(|error| {
                    AuthenticationError::new(format!("Decrypting handshake failed: {error}"))
                })?
            }
        };

        message.payload = Value::Bytes(decrypted.clone());
        message.payload_bytes = decrypted;
        Ok(())
    }

    fn on_device_inbox_message(&self, message: PhotonMessage, from: &Id) {
        self.try_on_device_inbox_message(message, from)
            .map_err(|error| warn!("{error}"))
            .ok();
    }

    async fn process_inbox_message(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        match message.message_type() {
            MessageType::Content  => {
                self.process_incoming_content_message(message, from).await
            }
            MessageType::State  => {
                self.process_state_message(message, &from).await
            }
            MessageType::Handshake  => {
                self.process_incoming_handshake(&message, &from).await;
                Ok(())
            }
            MessageType::Control  => {
                warn!("Ignoring control message from user inbox");
                Ok(())
            }
        }
    }

    async fn process_outbox_message(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        match message.message_type() {
            MessageType::Handshake => {
                self.process_outgoing_handshake(message, &from).await?;
                Ok(())
            }
            MessageType::Content => {
                self.on_outbox_content_message(message, from);
                Ok(())
            }
            _ => {
                warn!("Ignoring unsupported message type from user outbox");
                Ok(())
            }
        }
    }

    /*
    async fn on_handshake(&self, topic: Topic, message: PhotonMessage, from: &Id) {
        self.try_on_handshake(topic, message, *from)
            .await
            .map_err(|error| warn!("{error}"))
            .ok();
    }*/

    fn on_outbox_content_message(&self, message: PhotonMessage, from: &Id) {
        if from != self.user_id() {
            warn!("Outbox message has an unexpected sender");
            return;
        }
        self.process_outgoing_content_message(message, *from);
    }

    fn try_on_device_inbox_message(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        if message.message_type() != MessageType::Control {
            return Err(EncodingError::new(
                "Ignoring non-control message from device inbox"
            ));
        }
        if message.recipient() != self.user_id()
            && message.recipient() != self.device_id()
        {
            return Err(EncodingError::new(
                "Ignoring device RPC response for another device"
            ));
        }
        let Value::Bytes(payload) = message.payload else {
            return Err(EncodingError::new(
                "Rpc response payload is not binary, ignoring message"
            ));
        };

        self.process_control_message(&payload, from)
    }

    fn process_control_message(&self, payload: &[u8], from: &Id) -> Result<()> {
        let response = response::RpcResponse::deserialize(payload)?;
        trace!("Received rpc response: {}", response.id());
        self.pending_calls.respond(response, from)
    }

    async fn process_state_message(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        let Value::Bytes(payload) = &message.payload else {
            return Err(EncodingError::new("Notification payload is not binary"));
        };

        let notification = Notification::deserialize(payload)?;
        if notification.source() == self.user_id() &&
            notification.is_originated(self.device_id()) {

            // This notification was triggered by an RPC request from this device.
			// Since the local state was already updated via the RPC response,
			// this redundant notification can be safely ignored.
            trace!("The channel notification originated by this device, ignoring it");
            return Ok(());
        }

        if self.peer_id() == from {
            self.process_peer_notification(notification).await?;
            return Ok(());
        }

        let channel = match self.get_channel(from) {
            Some(c) => c,
            None => {
                let Body::ChannelJoin(info) = notification.body() else {
                    warn!("Received a notification for unknown channel {from}, ignoring it");
                    return Ok(());
                };

                if info.session_key().is_none() {
                    warn!("Recieved a channel join notification but missing session key from {from}, ignoring it");
                    return Ok(());
                }

                let channel = PhotonChannel::from(info, None);
                self.put_channel(&channel)?;
                self.get_channel(from).ok_or_else(|| {
                    StateError::new(&format!("Failed to create channel contact for {from}"))
                })?
            }
        };
        self.process_channel_notification(notification, channel)
            .await
            .map_err(|e| e.into())
    }

    async fn process_peer_notification(&self, notif: Notification) -> Result<()> {
        match notif.body() {
            Body::SessionNew(v) => {
                self.session_listener.on_new_session(v);
                Ok(())
            },
            Body::ContactSync(v) => {
                self.apply_contact_sync(v).await?;
                if self.connected.get() && !self.stopping.get() && !self.ready.replace(true) {
                    info!("Messaging session is ready");
                    self.notify_connection(|listener| listener.on_ready());
                }
                Ok(())
            },
            Body::ChannelCreate(ci) => {
                if ci.session_key().is_none() {
                    return Err(StateError::new(
                        &format!("Received a channel create notification but missing session key from {}, ignoring it",
                        self.peer_id()),
                    ));
                }

                trace!("Channel {} created", ci.channel_id());
                let session_key = ci.session_key().map(|v|v.to_vec());
                let new_channel = PhotonChannel::from(ci, session_key);

                // Save the channel to the local database immediately to provide a responsive UI.
				// We bypass the official 'contactsRevision' update here because this is a local-only
				// optimistic insert. The server will later broadcast a CONTACT_MUTATE notification
				// triggered by the device that initiated the original CHANNEL_CREATE request,
				// which will perform the formal, synchronized update of the contact list and revision state.

                let channel = match self.get_channel(ci.channel_id()) {
                    Some(existing) => {
                        trace!("Channel {} already exists, skip to save the channel",
                            ci.channel_id());
                        existing
                    }
                    None => {
                        // Created on another of this user's own devices: the session
                        // isn't registered on this one yet, so recover it from the
                        // self-encrypted session key before the channel can be saved
                        // or used (`put_channel` requires a registered session).
                        self.self_context.borrow_mut().reset_nonce();
                        let session_sk = self.self_context.borrow_mut()
                            .decrypt_into(new_channel.session_key())?;
                        let session_identity = CryptoIdentity::try_from(session_sk.as_slice())?;
                        self.session_contexts.lock().unwrap().insert(
                            *new_channel.id(),
                            self.session_context(*new_channel.id(), session_identity, true),
                        );
                        self.put_channel(&new_channel)?;
                        new_channel
                    }
                };

                self.channel_listener.on_channel_created(&(Box::new(channel) as Box<dyn ChannelHandler>));
                Ok(())
            },
            _ => {
                Err(StateError::new(
                    &format!("Received an unknown peer notification: ignoring it {notif:?}")
                ))
            }
        }
    }

    async fn process_channel_notification(&self, notif: Notification, mut channel: PhotonChannel) -> Result<()> {
        let channel_id = channel.id();
        match notif.body() {
            Body::ChannelDeletion => {
                trace!("Channel {} deleted", channel_id);
                // TODO: remove contact locally;
                self.remove_channel(channel.id()).await;
                self.channel_listener.on_channel_deleted(&Box::new(channel).into());
            }
            Body::ChannelJoin(info) => {
                trace!("Channel {} joined", channel_id);
                if self.get_channel(channel_id).is_none() {
                    // Save the channel to the local database immediately to provide a responsive UI.
					// We bypass the official 'contactsRevision' update here because this is a local-only
					// optimistic insert. The server will later broadcast a CONTACT_MUTATE notification
					// triggered by the device that initiated the original CHANNEL_CREATE request,
					// which will perform the formal, synchronized update of the contact list and revision state.

                    let _ = self.put_channel(&channel);
                } else {
                    // ready synced the channel contact.
                    // placeholder.
                }

                if !info.members().is_empty() {
                    let members: Vec<ChannelMember> = info.members()
                        .iter()
                        .cloned()
                        .map(|member| Box::new(member) as ChannelMember)
                        .collect();
                    if let Err(error) = self.repository
                        .borrow_mut()
                        .put_channel_members(channel_id, &members)
                    {
                        warn!("Failed to store channel members for {channel_id}: {error}");
                    }
                }
                self.channel_listener.on_joined_channel(&Box::new(channel).into());
            }
            Body::ChannelLeave => {
                trace!("Channel {} left", channel_id);
                self.remove_channel(channel.id()).await;
                // TODO: invalidate channel members cache for this channel
                self.channel_listener.on_left_channel(&Box::new(channel).into());
            }
            Body::ChannelOwnershipTransfer(new_owner) => {
                trace!("Channel {} ownership transfer to {}", channel_id, new_owner);

                let old_owner = *channel.owner();
                let channel = if !channel.has_member(&old_owner) || !channel.has_member(new_owner) {
                    // Not up-to-date: repair the gap by re-fetching the channel.
                    warn!("Looks like channel {} is outdated, try to refresh", channel_id);
                    self.refresh_channel(&channel).await?
                } else {
                    match self.repository.borrow_mut().update_channel_ownership(channel_id, &old_owner, new_owner) {
                        Ok(()) => {
                            let updated = channel.edit_channel().set_owner(*new_owner).build();
                            self.put_channel(&updated)?;
                            updated
                        }
                        Err(error) => {
                            warn!("Update channel ownership failed, looks like channel {} is outdated, try to refresh: {error}", channel_id);
                            self.refresh_channel(&channel).await?
                        }
                    }
                };
                self.channel_listener.on_channel_ownership_transferred(
                    &(Box::new(channel) as Box<dyn ChannelHandler>),
                    &old_owner,
                    new_owner,
                );
            }
            Body::ChannelSessionKeyRotate(encrypted_key) => {
                trace!("Channel {} session key rotation", channel_id);

                let channel = match self.get_channel(&channel_id) {
                    Some(channel) => channel,
                    None => {
                        warn!("Failed to get channel snapshot");
                        return Ok(());
                    }
                };
                if notif.source() != channel.owner() {
                    warn!("Channel session key rotation has an unauthorized source");
                    return Ok(());
                }
                let Some(context) = self.session_context_for(&channel_id) else {
                    warn!("Channel session key is missing");
                    return Ok(());
                };
                let key = match context.decrypt_into(&notif.source(), encrypted_key) {
                    Ok(key) => key,
                    Err(error) => {
                        warn!("Decrypting rotated channel session key failed: {error}");
                        return Ok(());
                    }
                };
                let session = match CryptoIdentity::try_from(key.as_slice()) {
                    Ok(session) => session,
                    Err(error) => {
                        warn!("Invalid rotated channel session key: {error}");
                        return Ok(());
                    }
                };
                // Store the rotated key self-encrypted, same as every other persisted
                // session key, not the form it arrived in (encrypted for channel
                // members via the old session's tx crypto context).
                self.self_context.borrow_mut().reset_nonce();
                let encrypted_for_self = self.self_context.borrow_mut().encrypt_into(&key)?;
                let mut updated = channel.edit_channel().set_session_key(encrypted_for_self).build();
                updated.set_session_id(*session.id());

                self.session_contexts
                    .lock()
                    .unwrap()
                    .insert(*channel_id, self.session_context(*channel_id, session, true));

                self.put_channel(&updated)?;
                self.channel_listener.on_channel_session_key_rotated(&(Box::new(updated) as Box<dyn ChannelHandler>));
                return Ok(());
            }

            Body::ChannelInfoUpdate(changes) => {
                trace!("Channel {} info updated: {}", channel_id, changes);

                let updated = channel
                    .edit_channel()
                    .patch(changes)
                    .build();

                self.put_channel(&updated)?;
                // TODO: put channel locally
                // invalidate cached channel.
                self.channel_listener.on_channel_updated(&Box::new(updated).into());
            }
            Body::ChannelMembersRoleUpdate(update) => {
                trace!("Channel {} members role updated: {} - {:?}",
                    channel_id,
                    update.role(),
                    update.member_ids()
                );

                let changed = channel.update_member_roles(update.member_ids(), update.role());
                self.put_channel(&channel)?;
                self.channel_listener
                    .on_channel_members_role_updated(&Box::new(channel).into(), &changed);
                return Ok(());
            }
            Body::ChannelMembersBan(ids) => {
                trace!("Channel {} members banned: {:?}", channel_id, ids);
                let changed = channel.update_member_roles(ids, Role::Banned);
                self.put_channel(&channel)?;
                self.channel_listener
                    .on_channel_members_banned(&Box::new(channel).into(), &changed);
                return Ok(());
            }
            Body::ChannelMembersUnban(ids) => {
                trace!("Channel {} members unbanned: {:?}", channel_id, ids);
                let mut channel = match self.get_channel(&channel_id) {
                    Some(channel) => channel,
                    None => {
                        warn!("Failed to get channel snapshot");
                        return Ok(());
                    }
                };
                let changed = channel.update_member_roles(ids, Role::Member);
                self.put_channel(&channel)?;
                self.channel_listener
                    .on_channel_members_unbanned(&Box::new(channel).into(), &changed);
                return Ok(());
            }
            Body::ChannelMembersRemove(ids) => {
                let mut channel = match self.get_channel(&channel_id) {
                    Some(channel) => channel,
                    None => {
                        warn!("Failed to get channel snapshot");
                        return Ok(());
                    }
                };
                let removed = channel.remove_members(ids);
                self.put_channel(&channel)?;
                self.channel_listener
                    .on_channel_members_removed(&Box::new(channel).into(), &removed);
                return Ok(());
            }
            Body::ChannelMemberJoin(member) => {
                if member.joined().duration_since(UNIX_EPOCH).is_err() {
                    warn!("Channel member timestamp is out of range");
                    return Ok(());
                }

                let mut channel = match self.get_channel(&channel_id) {
                    Some(channel) => channel,
                    None => {
                        warn!("Failed to get channel snapshot");
                        return Ok(());
                    }
                };
                channel.upsert_member(*member.id(), member.role(), member.joined());
                self.put_channel(&channel)?;
                self.channel_listener
                    .on_channel_member_joined(&(Box::new(channel) as Channel), &(Box::new(member.clone()) as ChannelMember));
                return Ok(());
            }
            Body::ChannelMemberLeave(id) => {
                let mut channel = match self.get_channel(&channel_id) {
                    Some(channel) => channel,
                    None => {
                        warn!("Failed to get channel snapshot");
                        return Ok(());
                    }
                };
                let member = channel.members().into_iter().find(|member| member.id() == id);
                channel.remove_members(&[*id]);
                self.put_channel(&channel)?;
                if let Some(member) = member {
                    self.channel_listener
                        .on_channel_member_left(&(Box::new(channel) as Channel), &member);
                }
                return Ok(());
            }
            body => warn!("Unsupported channel notification: {body:?}"),
        }

        Ok(())
    }

    /// Re-fetches a channel's info and members from the service, used to repair
    /// a channel snapshot that a notification found to be out of date (e.g. an
    /// ownership transfer referring to a member this device hasn't seen yet).
    pub(super) async fn refresh_channel(&self, channel: &PhotonChannel) -> Result<PhotonChannel> {
        let channel_id = *channel.id();
        let request = RPCRequest::get_channel_info(0);
        let result = self.send_rpccall(&channel_id, request).await?;
        let response::Value::ChannelInfo(ci) = result else {
            return Err(EncodingError::new("RPC result is not channel info").into());
        };

        let mut contact = channel.photon_contact();
        contact.set_name(ci.name().map(ToString::to_string));
        contact.set_updated_at(ci.updated_at());
        let updated = PhotonChannel::new(
            contact,
            *ci.owner_id(),
            *channel.session_id(),
            ci.permission(),
            ci.notice().map(ToString::to_string),
            ci.announce(),
            ci.members().to_vec(),
        );
        self.put_channel(&updated)?;
        Ok(updated)
    }

    /// Decodes a contact, keeping the session key and channel metadata that the
    /// opaque document carries for the contact kinds that have them.
    pub(super) fn decode_contact(&self, opaque: &OpaqueContact) -> Result<DecodedContact> {
        let (contact, plaintext) = opaque.decode_into(&self.user_identity)?;
        let session = contact.session(&self.user_identity)?;
        let channel = match contact.contact_type() {
            ContactType::Channel => {
                let info: ChannelContactInfo = serde_cbor::from_slice(&plaintext).map_err(|e| EncodingError::new(format!("Malformed channel contact: {e}")))?;
                let session_id = *session
                    .as_ref()
                    .ok_or_else(|| EncodingError::new("Channel session is missing"))?
                    .id();
                Some(PhotonChannel::new(
                    contact.clone(),
                    info.owner,
                    session_id,
                    Permission::try_from(info.permission)?,
                    info.notice,
                    info.announced,
                    Vec::new(),
                ))
            }
            _ => None,
        };
        Ok(DecodedContact {
            contact,
            session,
            channel,
        })
    }

    async fn apply_contact_sync(&self, sync: &ContactSync) -> Result<()> {
        let revision = self.contacts_revision.get();
        if matches!(sync.kind(), SyncType::UpToDate) {
            if sync.revision() == revision {
                return Ok(());
            }
            self.contact_sync_needed.set(sync.revision() > revision);
            return Err(StateError::new(
                "Contact sync revision does not match local state",
            ));
        }

        let previous = self
            .repository
            .borrow_mut()
            .load_contacts()
            .unwrap_or_default()
            .into_iter()
            .map(|contact| (*contact.id(), Box::new(contact) as Contact))
            .collect::<HashMap<_, _>>();
        let sessions = self.session_contexts.lock().unwrap().clone();
        let channels = self
            .repository
            .borrow_mut()
            .load_channels()
            .unwrap_or_default()
            .into_iter()
            .map(|channel| (*channel.id(), channel))
            .collect::<HashMap<_, _>>();
        let mut state = ContactState {
            contacts: previous.clone(),
            sessions,
            channels,
            revision,
        };
        match sync.kind() {
            SyncType::Snapshot => self.apply_snapshot(sync, &mut state)?,
            SyncType::Delta => self.apply_delta(sync, &mut state)?,
            SyncType::UpToDate => unreachable!("handled above"),
        }
        self.commit_contact_state(previous, state)
            .map_err(|e| e.into())
    }

    pub(super) fn apply_snapshot(&self, sync: &ContactSync, state: &mut ContactState) -> Result<()> {
        if sync.revision() < state.revision {
            return Err(StateError::new("Invalid or outdated contact snapshot"));
        }
        state.contacts.clear();
        state.sessions.clear();
        state.channels.clear();
        for opaque in sync.contacts() {
            if opaque.revision() > sync.revision() {
                return Err(EncodingError::new(
                    "Contact revision exceeds snapshot revision",
                ));
            }
            let DecodedContact {
                contact,
                session,
                channel,
            } = match self.decode_contact(opaque) {
                Ok(contact) => contact,
                Err(error) => {
                    warn!("Skipping invalid contact {}: {error}", opaque.id());
                    continue;
                }
            };
            let id = *contact.id();
            if state.contacts.contains_key(&id) {
                return Err(EncodingError::new("Snapshot contains duplicate contacts"));
            }
            let is_channel = channel.is_some();
            if let Some(session) = session {
                state
                    .sessions
                    .insert(id, self.session_context(id, session, is_channel));
            }
            if let Some(channel) = channel {
                state.channels.insert(id, channel);
            }
            state.contacts.insert(id, Box::new(contact));
        }
        state.revision = sync.revision();
        Ok(())
    }

    pub(super) fn superseded_mutations(mutations: &[ContactMutation]) -> Vec<bool> {
        let mut superseded = vec![false; mutations.len()];
        let mut touched = HashSet::new();
        let mut cleared = false;
        for (index, mutation) in mutations.iter().enumerate().rev() {
            match mutation.data() {
                Some(MutationData::Add(opaque) | MutationData::Update(opaque)) => {
                    superseded[index] = cleared || !touched.insert(*opaque.id());
                }
                Some(MutationData::Remove(ids)) => touched.extend(ids.iter().copied()),
                None | Some(MutationData::Clear) => cleared = true,
            }
        }
        superseded
    }

    pub(super) fn apply_delta(&self, sync: &ContactSync, state: &mut ContactState) -> Result<()> {
        let original_revision = self.contacts_revision.get();
        let superseded = Self::superseded_mutations(sync.mutations());
        for (index, mutation) in sync.mutations().iter().enumerate() {
            if mutation.revision() < 0 {
                return Err(EncodingError::new("Contact mutation revision is negative"));
            }
            if mutation.revision() < state.revision {
                continue;
            }
            if mutation.revision() > state.revision {
                self.contact_sync_needed.set(true);
                return Err(StateError::new("Contact delta has a revision gap"));
            }
            let next = state
                .revision
                .checked_add(1)
                .ok_or_else(|| EncodingError::new("Contact revision is exhausted"))?;

            match mutation.data() {
                Some(MutationData::Add(opaque) | MutationData::Update(opaque)) => {
                    if opaque.revision() != next {
                        return Err(EncodingError::new(
                            "Contact mutation revision is inconsistent",
                        ));
                    }
                    if superseded[index] {
                        state.revision = next;
                        continue;
                    }
                    let DecodedContact {
                        contact,
                        session,
                        channel,
                    } = match self.decode_contact(opaque) {
                        Ok(contact) => contact,
                        Err(error) => {
                            warn!("Skipping invalid contact {}: {error}", opaque.id());
                            state.revision = next;
                            continue;
                        }
                    };
                    let id = *contact.id();
                    state.sessions.remove(&id);
                    let is_channel = channel.is_some();
                    if let Some(session) = session {
                        state
                            .sessions
                            .insert(id, self.session_context(id, session, is_channel));
                    }
                    state.channels.remove(&id);
                    if let Some(channel) = channel {
                        state.channels.insert(id, channel);
                    }
                    state.contacts.insert(id, Box::new(contact));
                }
                Some(MutationData::Remove(ids)) => {
                    for id in ids {
                        state.contacts.remove(&id);
                        state.sessions.remove(&id);
                        state.channels.remove(id);
                    }
                }
                None | Some(MutationData::Clear) => {
                    state.contacts.clear();
                    state.sessions.clear();
                    state.channels.clear();
                }
            }
            state.revision = next;
        }
        if state.revision < sync.revision()
            || (state.revision > sync.revision() && state.revision > original_revision)
        {
            return Err(EncodingError::new(
                "Contact delta final revision is inconsistent",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_contact_state(
        &self,
        previous: HashMap<Id, Contact>,
        state: ContactState,
    ) -> Result<()> {
        if self.contacts_revision.get() > state.revision {
            return Ok(());
        }
        let removed: Vec<_> = previous
            .keys()
            .filter(|id| !state.contacts.contains_key(id))
            .copied()
            .collect();
        let added: Vec<Contact> = state
            .contacts
            .values()
            .filter(|contact| !previous.contains_key(contact.id()))
            .cloned()
            .collect();
        let updated: Vec<Contact> = state
            .contacts
            .values()
            .filter(|contact| Self::contact_changed(previous.get(contact.id()), contact))
            .cloned()
            .collect();

        for id in &removed {
            let _ = self.repository.borrow_mut().remove_contact_locally(id);
        }
        for (id, contact) in &state.contacts {
            if let Some(channel) = state.channels.get(id) {
                let photon = channel.photon_contact();
                let session_key = match photon.session_key() {
                    Some(key) => key.to_vec(),
                    None => match state.sessions.get(id) {
                        Some(session) => self
                            .user_identity
                            .encrypt_into(self.user_id(), session.private_key())?,
                        None => Vec::new(),
                    },
                };
                self.repository
                    .borrow_mut()
                    .store_channel(state.revision, channel, &session_key)?;
                continue;
            }
            let mut editor = contact.edit().set_revision(state.revision);
            if let Some(session) = state.sessions.get(id) {
                editor = editor.set_session_key(Some(
                    self.user_identity
                        .encrypt_into(self.user_id(), session.private_key())?,
                ));
            }
            let photon = PhotonContact::from_contact(&editor.build());
            let session_key = photon.session_key().map(|key| key.to_vec());
            self.repository.borrow_mut().put_contact(
                state.revision,
                &photon,
                session_key.as_deref(),
            )?;
        }

        *self.blocked_contacts.lock().unwrap() = state
            .contacts
            .values()
            .filter(|contact| contact.is_blocked())
            .map(|contact| *contact.id())
            .collect();
        *self.session_contexts.lock().unwrap() = state.sessions;
        self.contacts_revision.set(state.revision);

        if !removed.is_empty() {
            self.contact_listener.on_contacts_removed(&removed);
        }
        for contact in added {
            self.contact_listener.on_contact_added(&contact);
        }
        if !updated.is_empty() {
            self.contact_listener.on_contacts_updated(&updated);
        }
        Ok(())
    }

    pub(super) fn contact_changed(old: Option<&Contact>, new: &Contact) -> bool {
        let Some(old) = old else {
            return false;
        };
        old.revision() != new.revision()
            || old.updated_at() != new.updated_at()
            || old.is_blocked() != new.is_blocked()
            || old.is_muted() != new.is_muted()
            || old.name() != new.name()
            || old.remark() != new.remark()
            || old.tags() != new.tags()
            || old.contact_type() != new.contact_type()
    }

    async fn process_incoming_handshake(&self, message: &PhotonMessage, from: &Id) {
        let Ok(hs) = Handshake::deserialize(&message.payload_bytes) else {
            warn!("Deserialization of handshake failed from {from}, ignoring message");
            return;
        };

        match hs.body {
            HandshakeType::FriendRequest(hello) => {
                trace!("Received a friend request from {from}");

                if hs.timestamp < 0 {
                    warn!("Received friend request with negative timestamp from {from}, ignoring it");
                    return;
                }

                if let Some(contact) = self.get_contact(from).await {
                    if contact.is_blocked() {
                        warn!("Received friend request from blocked contact {from}, ignoring it");
                        return;
                    }

                    if contact.contact_type() == ContactType::Friend {
                        debug!("Received friend request from existing friend {from}, ignoring it");
                        return;
                    }

                    if contact.contact_type() == ContactType::Channel {
                        debug!("INTERNAL ERROR!!! Received friend request from existing channel {from}, ignoring it");
                        return;
                    }
                }

                let fr = PhotonFriendRequest::received(
                    *from,
                    *from,
                    hello.clone(),
                    hs.timestamp,
                    boson_api::as_ms!(SystemTime::now()) as i64,
                );
                let _ = self.put_friend_request(&fr)
                    .map_err(|error| warn!("Cache friend request failed: {error}"));

                self.friend_request_listener.on_friend_request(
                    &fr.user_id(),
                    hello.as_deref()
                );
            }
            HandshakeType::FriendRequestAccept(session_key) => {
                trace!("Received friend request acceptance from {from}");
                let Some(mut fr) = self.friend_request(from) else {
                    warn!("No matched pending friend request found from {from}, ignoring it");
                    return;
                };
                let accepted_at = PhotonFriendRequest::not_in_the_future(
                    hs.timestamp,
                    boson_api::as_ms!(SystemTime::now()) as i64,
                );
                if fr.is_expired_at(SystemTime::now()) {
                    warn!("Received a friend request acceptance from {from} against an expired request, ignoring it");
                    return;
                }
                if session_key.len() != signature::PrivateKey::BYTES {
                    warn!("Received a friend request acceptance with invalid session key length from {from}, ignoring it");
                    return;
                }

                if let Some(contact) = self.get_contact(from).await {
                    if contact.contact_type() == ContactType::Channel {
                        warn!("INTERNAL ERROR!!! Received a friend request acceptance from a channel {from}, ignoring it");
                        return;
                    }
                    if contact.is_blocked() {
                        warn!("Received a friend request acceptance from a blocked user {from}, ignoring it");
                        return;
                    }

                    fr.accept(accepted_at);
                    trace!("Update friend request status for {from} to accepted");

                    if contact.contact_type() == ContactType::Friend {
                        // Another device of this user got here first and the new friend has already
						// been synced to this one; only the request record is left to update.
                        trace!("{from} is already a friend, only updating the request record");

                        let _ = self.put_friend_request(&fr);
                        return;
                    }
                };

                // All user devices receive the 'accept' notification.
                // Since we are lacking a leader-election mechanism between devices, all devices attempt
                // to add the new contact upon receiving the 'accept' notification.
                // The contact synchronization service handles concurrency by ensuring only
                // the first received update is applied and synchronized to all devices.

                trace!("Trying to add the contact {}...", from);
                let encrypted_key = match self.self_context.borrow_mut().encrypt_into(&session_key) {
                    Ok(key) => key,
                    Err(error) => {
                        warn!("Failed to encrypt friend session key for {from}: {error}");
                        return;
                    }
                };
                let result = self.add_friend_internal(*from, encrypted_key, None).await;
                match result {
                    Ok(_) => {
                        trace!("Synced new friend {from} cross devices");
                        match CryptoIdentity::try_from(session_key.as_slice()) {
                            Ok(session_identity) => {
                                self.session_contexts.lock().unwrap().insert(
                                    *from,
                                    self.session_context(*from, session_identity, false),
                                );
                            }
                            Err(error) => warn!("Invalid friend session key from {from}: {error}"),
                        }
                    }
                    Err(error) => warn!("Failed to sync new friend {} cross devices: {error}", from),
                }
                fr.accept(accepted_at);
                let _ = self.put_friend_request(&fr);
                self.friend_request_listener.on_friend_request_accepted(from);
            }
        }
    }

    async fn process_outgoing_handshake(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        if message.is_originated(self.device_id()) {
            trace!("The handshake {} to {} is redundant (or originated from self), ignoring it.",
                MessageOps::id(&message), from);
            return Ok(());
        }

        let handshake = Handshake::deserialize(&message.payload_bytes)?;
        match handshake.body {
            HandshakeType::FriendRequest(hello) => {
                trace!("Friend request sent to {}", message.recipient());
                let request = PhotonFriendRequest::received(
                    *message.recipient(),
                    *self.user_identity.id(),
                    hello,
                    handshake.timestamp,
                    boson_api::as_ms!(SystemTime::now()) as i64
                );
                let _ = self.put_friend_request(&request);
                return Ok(());
            }
            HandshakeType::FriendRequestAccept(session_key) => {
                trace!("Acecept friend request from {}", message.recipient());

                let from = message.recipient();
                let Some(mut fr) = self.friend_request(&message.recipient()) else {
                    warn!("No matched pending friend request found for {from}, ignoring it");
                    return Ok(());
                };
                if fr.is_outgoing() {
                    warn!("Received friend request acceptance from {from} for an outgoing request, ignoring it");
                    return Ok(());
                }
                if fr.is_accepted() {
                    warn!("Received friend request acceptance from {from} for an already accepted request, ignoring it");
                    return Ok(());
                }

                let accepted_at = PhotonFriendRequest::not_in_the_future(
                    handshake.timestamp,
                    boson_api::as_ms!(SystemTime::now()) as i64
                );
                fr.accept(accepted_at);
                let _ = self.put_friend_request(&fr);
            }
        }
        Ok(())
    }

    async fn conversation_or_create(&self, id: &Id) -> Result<PhotonConversation> {
        if let Some(conversation) = self.conversation(id).await {
            return Ok(conversation);
        }
        let contact = self
            .get_contact(id)
            .await
            .ok_or_else(|| StateError::new(format!("No contact found for {id}")))?;
        Ok(PhotonConversation::new(Box::new(contact) as Contact))
    }

    async fn process_incoming_content_message(
        &self,
        mut message: PhotonMessage,
        from: &Id,
    ) -> Result<()> {
        if from == self.user_id() {
            return Err(StateError::new("Received a message from self").into());
        }

        let content = match PhotonContent::parse(&message.payload_bytes) {
            Ok(content) => content,
            Err(error) => {
                warn!("Malformed incoming content: {error}");
                return Err(error);
            }
        };
        let content_message = message.set_content(content);
        message.set_from(*from);

        if let Err(error) = self.repository.borrow_mut().put_message(&mut message) {
            error!(
                "Failed to persist incoming message {}: {error}",
                MessageOps::id(&message)
            );
        }

        let Some(conversation_id) = message.conversation_id().copied() else {
            return Err(StateError::new("Conversation ID is missing").into());
        };

        match self.conversation_or_create(&conversation_id).await {
            Ok(mut conversation) => {
                if let Err(error) = conversation.update(content_message.clone()) {
                    warn!("Failed to update conversation {conversation_id}: {error}");
                }
                let _ = self.repository.borrow_mut().put_conversation(conversation);
            }
            Err(error) => {
                warn!("Failed to get or create conversation {conversation_id}: {error}");
            }
        }

        self.message_listener.on_message(&content_message);
        Ok(())
    }

    fn process_outgoing_content_message(&self, mut message: PhotonMessage, from: Id) {
        let contact_id = message.conversation_id().copied().unwrap_or(*message.recipient());

        let content = match PhotonContent::parse(&message.payload_bytes) {
            Ok(content) => content,
            Err(error) => {
                warn!("Malformed outgoing content: {error}");
                return;
            }
        };
        message.content = Some(content);
        message.set_from(from);
        message.set_sent(boson_api::as_ms!(SystemTime::now()) as i64);

        if message.is_originated(self.device_id()) {
            trace!(
                "Message {} sent to {} originated from this device",
                MessageOps::id(&message),
                contact_id
            );
            self.message_listener.on_sent(&(Box::new(message) as Message));
            return;
        }

        trace!(
            "Message {} sent to {} originated from another device",
            MessageOps::id(&message),
            contact_id
        );
        if let Err(error) = self.repository.borrow_mut().put_message(&mut message) {
            warn!("Failed to persist outgoing message: {error}");
        }
        self.message_listener.on_sent(&(Box::new(message) as Message));
    }

    pub(super) fn message_id(device_id: &Id, timestamp: i64) -> Result<Id> {
        let mut digest = Sha256::new();
        digest.update(device_id.as_bytes());
        digest.update(timestamp.to_be_bytes());

        let hash = digest.finalize();
        Id::try_from_bytes(hash.as_slice())
            .map_err(|error| EncodingError::new(error.to_string()).into())
    }

    pub(super) fn notify_connection(&self, callback: impl Fn(&dyn ConnectionListener)) {
        callback(self.connection_listener.as_ref());
    }

    /// Asks the service for everything that changed since our revision.
    pub(super) async fn sync_contacts(&self) -> Result<()> {
        // 1. Validate: nothing to check, the sync starts from our revision.
        let revision = self.contacts_revision.get();

        // 2. Build the request. A fresh device asks for a snapshot rather than
        // replaying the full contact history.
        let requested = if revision == 0 { i32::MAX } else { revision };
        let request = RPCRequest::contact_sync(0, requested);

        // 3. Send it to the home peer and 4. decode the delta it reports.
        let result = self.send_rpccall(self.peer_id(), request).await?;
        let response::Value::ContactSync(sync) = result else {
            return Err(EncodingError::new("RPC result is not a contact sync").into());
        };

        // 5. Apply the delta to the local state.
        self.apply_contact_sync(&sync).await
    }
}
