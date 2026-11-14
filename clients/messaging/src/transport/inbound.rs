use log::trace;

use super::prelude::*;
use super::outbound::update_conversation;

use crate::internal::{
    notification::{Body, Notification},
};

use crate::{
    MessageOps,
    MessageOriginated,
    ChannelOps
};
use crate::rpc::response::RpcResponse;
use crate::MessageType;

use crate::internal::photon_message::PhotonMessage;

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
                    recipient.clone()
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
            warn!("Failed to check/decrypt incoming message: {error}");
            return;
        }

        match topic {
            Topic::DeviceInbox => self.on_device_inbox_message(message, &from),
            Topic::UserInbox => self.on_inbox_message(message, &from).await,
            Topic::UserOutbox => self.on_outbox_message(message, &from).await,
            Topic::DeviceOutbox => {
                warn!("Ignoring message from device outbox");
            }
        }
    }

    async fn check_and_decrypt_message(&self, message: &mut PhotonMessage) -> Result<()> {
        let from = *message.from_or_throw()?;
        let recipient = message.recipient();
        let message_type = message.message_type();
        let payload = match &message.payload {
            Value::Bytes(payload) => payload.clone(),
            _ => return Err(EncodingError::new("Message payload is not binary").into()),
        };

        let decrypted = match message_type {
            MessageType::Content => {
                let Some(conversation_id) = message.conversation_id() else {
                    return Err(StateError::new("Conversation ID is not set").into());
                };
                let Some(contact) = self.contact(&conversation_id).await else {
                    return Err(StateError::new(format!(
                        "Received a message for unknown contact {conversation_id}"
                    )).into());
                };

                if contact.is_blocked() {
                    return Err(StateError::new("Received a message from a blocked contact").into());
                }
                if contact.contact_type() == ContactType::Channel {
                    // TODO: Channel content uses the channel conversation crypto context.
                } else if contact.contact_type() == ContactType::Auto {
                    return Err(StateError::new("Received a message from a contact of no friend.").into());
                }

                // A blocked user's messages in a channel are still delivered: the channel is a shared
				// conversation, and dropping one member's messages would leave holes (replies without
				// what they answer) and a view that differs from the other members'.

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
                } else if recipient == self.user_id() {
                    self.user_identity.decrypt_into(&from, &payload).map_err(|error| {
                        AuthenticationError::new(format!("Decrypting {message_type:?} failed: {error}"))
                    })?
                } else {
                    let channel = self.channel(&recipient).await.ok_or_else(|| {
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
                let friend_id = if from == *self.user_id() { recipient } else { &from };
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

    async fn on_inbox_message(&self, message: PhotonMessage, from: &Id) {
        match message.message_type() {
            MessageType::State  => {
                self.on_state_message(message, &from).await
            }
            MessageType::Handshake  => {
                self.on_handshake(Topic::UserInbox, message, &from).await
            }
            MessageType::Content  => {
                self.on_inbox_content_message(message, &from)
            }
            MessageType::Control  => {
                warn!("Ignoring control message from user inbox");
            }
            _ => {
                warn!("Ignoring unsupported message type from user inbox");
            }
        }
    }

    async fn on_outbox_message(&self, message: PhotonMessage, from: &Id) {
        match message.message_type() {
            MessageType::Handshake => {
                self.on_handshake(Topic::UserOutbox, message, &from).await
            }
            MessageType::Content => {
                self.on_outbox_content_message(message, from);
            }
            _ => {
                warn!("Ignoring unsupported message type from user outbox");
            }
        }
    }

    async fn on_state_message(&self, message: PhotonMessage, from: &Id) {
        self.try_on_state_message(message, from)
            .await
            .map_err(|error| warn!("{error}"))
            .ok();
    }

    async fn on_handshake(&self, topic: Topic, message: PhotonMessage, from: &Id) {
        self.try_on_handshake(topic, message, *from)
            .await
            .map_err(|error| warn!("{error}"))
            .ok();
    }

    fn on_inbox_content_message(&self, message: PhotonMessage, from: &Id) {
        if from == self.user_id() {
            warn!("Ignoring content message from user inbox that was sent by this user");
            return;
        }
        self.process_incoming_content_message(message, *from);
    }

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
        let deserded = serde_cbor::from_slice::<RpcResponse>(payload);
        let response = deserded.map_err(|e| EncodingError::new(
            &format!("Failed to decode RPC response: {e}")
        ))?;

        trace!("Received RPC response {}", response.id());

        self.pending_calls.respond(response, from)
    }

    async fn try_on_state_message(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        let Value::Bytes(payload) = &message.payload else {
            return Err(EncodingError::new("Notification payload is not binary"));
        };
        self.process_notification(payload, from).await
    }


    async fn process_notification(&self, payload: &[u8], from: &Id) -> Result<()> {
        let notif = serde_cbor::from_slice::<Notification>(payload)
                .map_err(|e| EncodingError::new(
                    &format!("Malformed notification: {e}")
                ))?;

        let timestamp = notif.timestamp() as i64;
        let originated = RpcServer::message_id(self.device_id(), timestamp)
            .map(|ref id| notif.id() == id)
            .unwrap_or(false);

        if notif.source() == self.user_id()
            && notif.is_channel_notification()
            && originated
        {
            trace!("Ignoring the channel notification originated by this device");
            return Ok(());
        }

        if self.peer_id() == from {
            self.process_peer_notification(notif).await
        } else {
            let channel = match self.channel(from).await {
                Some(c) => c,
                None => {
                    let Body::ChannelJoin(info) = notif.body() else {
                        return Err(StateError::new(
                            &format!("Received a notification for unknown channel {from}, ignoring it {notif:?}"),
                        ));
                    };

                    if info.session_key().is_none() {
                        return Err(StateError::new(
                            &format!("Recieved a channel join notification but missing session key from {from}, ignoring it {notif:?}"),
                        ));
                    }

                    self.put_channel(&self.channel_from_info(info)?.0).await?;
                    self.channel(from).await.ok_or_else(|| {
                        StateError::new(&format!("Failed to create channel contact for {from}"))
                    })?
                }
            };
            self.process_channel_notification(notif, &channel).await
        }
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
            Body::ChannelCreate(channel_info) => {
                if channel_info.session_key().is_none() {
                    return Err(StateError::new(
                        &format!("Received a channel create notification but missing session key from {}, ignoring it",
                        self.peer_id()),
                    ));
                }

                trace!("Channel {} created", channel_info.channel_id());
                let (new_channel, _) = self.channel_from_info(channel_info)?;

                let channel = match self.channel(channel_info.channel_id()).await {
                    Some(v) => {
                        trace!("Channel {} already exists, ignoring duplicate create channel notification",
                            channel_info.channel_id());
                        v
                    }
                    None => {
                        self.put_channel(&new_channel).await?;
                        new_channel
                    }
                };

                if !channel_info.members().is_empty() {
                    let members = channel_info.members().iter()
                        .map(|member| Box::new(member.clone()) as ChannelMember)
                        .collect::<Vec<_>>();
                    self.repository
                        .borrow_mut()
                        .put_channel_members(channel_info.channel_id(), &members)?;
                }
                self.channel_listener.on_channel_created(&channel);
                Ok(())
            },
            _ => {
                Err(StateError::new(
                    &format!("Received an unknown peer notification: ignoring it {notif:?}")
                ))
            }
        }
    }

    async fn process_channel_notification(
        &self,
        notif: Notification,
        channel: &Channel,
    ) -> Result<()> {
        let channel_id = channel.id();
        match notif.body() {
            Body::ChannelDeletion => {
                trace!("Channel {} deleted", channel_id);
                self.invalidate_channel(channel.id()).await;
                self.channel_listener.on_channel_deleted(&channel);
            }
            Body::ChannelJoin(info) => {
                trace!("Channel {} joined", channel_id);
                if self.channel(channel_id).await.is_none() {
                    self.put_channel(channel).await?;
                }

                if info.members().is_empty() {
                    trace!("Channel {} has no members", channel_id);
                } else {
                    let members = info.members().iter()
                        .map(|member| Box::new(member.clone()) as ChannelMember)
                        .collect::<Vec<_>>();
                    self.repository
                        .borrow_mut()
                        .put_channel_members(channel_id, &members)?;
                }
                self.channel_listener.on_joined_channel(channel);
            }
            Body::ChannelLeave => {
                trace!("Channel {} left", channel_id);
                self.invalidate_channel(channel.id()).await;
                self.channel_listener.on_left_channel(channel);
            }
            Body::ChannelSessionKeyRotate(encrypted_key) => {
                trace!("Channel {} session key rotation", channel_id);

                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
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
                channel.set_session_id(*session.id());
                self.session_contexts
                    .lock()
                    .unwrap()
                    .insert(*channel_id, self.session_context(*channel_id, session, true));

                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener.on_channel_session_key_rotated(&channel);
                return Ok(());
            }
            Body::ChannelOwnershipTransfer(new_owner) => {
                trace!("Channel {} ownership transfer", channel_id);
                /*
                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                let old_owner = *channel.owner();
                channel.set_owner(*new_owner);
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener
                    .on_channel_ownership_transferred(&channel, &old_owner, new_owner);
                return Ok(());
                */
            }
            Body::ChannelInfoUpdate(changes) => {
                trace!("Channel {} info updated ", channel.id());

                let mut updated = self.channel_snapshot(channel_id)?;
                if let Some(permission) = changes.get_permission() {
                    updated.set_permission(permission);
                }
                if let Some(name) = changes.get_name() {
                    updated.set_name(name.map(ToString::to_string));
                }
                if let Some(notice) = changes.get_notice() {
                    updated.set_notice(notice.map(ToString::to_string));
                }
                if let Some(announced) = changes.get_announced() {
                    updated.set_announced(announced);
                }
                self.put_channel(&updated).await?;
                self.channel_listener.on_channel_updated(&updated);
            }
            Body::ChannelMembersRoleUpdate(update) => {
                trace!("Channel {} members role updated: {} - {:?}",
                    channel_id,
                    update.role(),
                    update.member_ids()
                );

                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                let changed = channel.update_member_roles(update.member_ids(), update.role());
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener
                    .on_channel_members_role_updated(&channel, &changed);
                return Ok(());
            }
            Body::ChannelMembersBan(ids) => {
                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                let changed = channel.update_member_roles(ids, Role::Banned);
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener
                    .on_channel_members_banned(&channel, &changed);
                return Ok(());
            }
            Body::ChannelMembersUnban(ids) => {
                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                let changed = channel.update_member_roles(ids, Role::Member);
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener
                    .on_channel_members_unbanned(&channel, &changed);
                return Ok(());
            }
            Body::ChannelMembersRemove(ids) => {
                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                let removed = channel.remove_members(ids);
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener
                    .on_channel_members_removed(&channel, &removed);
                return Ok(());
            }
            Body::ChannelMemberJoin(member) => {
                if member.joined().duration_since(UNIX_EPOCH).is_err() {
                    warn!("Channel member timestamp is out of range");
                    return Ok(());
                }

                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                channel.upsert_member(*member.id(), member.role(), member.joined());
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                self.channel_listener
                    .on_channel_member_joined(&channel, &(Box::new(member.clone()) as ChannelMember));
                return Ok(());
            }
            Body::ChannelMemberLeave(id) => {
                let mut channel = match self.channel_snapshot(&channel_id) {
                    Ok(channel) => channel,
                    Err(error) => {
                        warn!("Failed to get channel snapshot: {error}");
                        return Ok(());
                    }
                };
                let member = channel.members().into_iter().find(|member| member.id() == id);
                channel.remove_members(&[*id]);
                self.channels_cache.borrow_mut().insert(*channel_id, channel.clone());
                if let Some(member) = member {
                    self.channel_listener
                        .on_channel_member_left(&channel, &member);
                }
                return Ok(());
            }
            body => warn!("Unsupported channel notification: {body:?}"),
        }

        Ok(())
    }

    /// Decodes a contact, keeping the session key and channel metadata that the
    /// opaque document carries for the contact kinds that have them.
    pub(super) fn decode_contact(&self, opaque: &OpaqueContact) -> Result<DecodedContact> {
        let (data, plaintext) = opaque.decode_into(&self.user_identity)?;
        let contact = data.contact(opaque.revision())?;
        let session = data.session(&self.user_identity)?;
        let channel = match contact.contact_type() {
            ContactType::Channel => {
                let info: ChannelContactInfo = serde_cbor::from_slice(&plaintext).map_err(|e| EncodingError::new(format!("Malformed channel contact: {e}")))?;
                let session_id = *session
                    .as_ref()
                    .ok_or_else(|| EncodingError::new("Channel session is missing"))?
                    .id();
                Some(Box::new(PhotonChannel::new(
                    PhotonContact::try_from(data.clone())?,
                    info.owner,
                    session_id,
                    Permission::try_from(info.permission)?,
                    info.notice,
                    info.announced,
                    Vec::new(),
                )) as Channel)
            }
            _ => None,
        };
        Ok(DecodedContact {
            contact,
            session,
            channel,
        })
    }

    async fn enqueue_contact_sync(&self, sync: &ContactSync) -> Result<()> {
        self.apply_contact_sync(sync).await
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

        let previous = self.contacts_cache.borrow().clone();
        let mut state = ContactState {
            contacts: previous.clone(),
            sessions: self.session_contexts.lock().unwrap().clone(),
            channels: self.channels_cache.borrow().clone(),
            revision,
        };
        match sync.kind() {
            SyncType::Snapshot => self.apply_snapshot(sync, &mut state)?,
            SyncType::Delta => self.apply_delta(sync, &mut state)?,
            SyncType::UpToDate => unreachable!("handled above"),
        }
        self.commit_contact_state(previous, state)
    }

    /// Replaces the whole contact book with a snapshot.
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
            } = self.decode_contact(&opaque)?;
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
            state.contacts.insert(id, contact);
        }
        state.revision = sync.revision();
        Ok(())
    }

    /// Flags the mutations whose effect a later mutation of the same delta
    /// fully replaces, so they do not need to be decrypted.
    ///
    /// A fresh client replays the whole history of the account; decrypting
    /// every superseded add/update made connecting scale with that history.
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

    /// Replays an ordered list of contact mutations.
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
                    } = self.decode_contact(opaque)?;
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
                    state.contacts.insert(id, contact);
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

    /// Publishes a validated contact book and tells the listeners what moved.
    pub(super) fn commit_contact_state(
        &self,
        previous: HashMap<Id, Contact>,
        state: ContactState,
    ) -> Result<()> {
        let removed: Vec<_> = previous
            .keys()
            .filter(|id| !state.contacts.contains_key(id))
            .copied()
            .collect();
        let added: Vec<_> = state
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

        *self.blocked_contacts.lock().unwrap() = state
            .contacts
            .values()
            .filter(|contact| contact.is_blocked())
            .map(|contact| *contact.id())
            .collect();
        *self.contacts_cache.borrow_mut() = state.contacts;
        *self.session_contexts.lock().unwrap() = state.sessions;
        *self.channels_cache.borrow_mut() = state.channels;
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

    /// Whether a contact differs from its stored copy in anything a listener
    /// would report. Contacts absent from `old` are additions, not updates.
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

    async fn try_on_handshake(
        &self,
        topic: Topic,
        message: PhotonMessage,
        from: Id
    ) -> Result<()> {
        let timestamp = message.created_at_u64() as i64;
        let originated_here = match Self::message_id(self.device_id(), timestamp) {
            Ok(id) => MessageOps::id(&message) == &id,
            Err(error) => {
                return Err(StateError::new("Failed to derive handshake message id").into());
            }
        };
        if matches!(topic, Topic::UserOutbox) && originated_here {
            return Ok(());
        }
        let friend_id = if matches!(topic, Topic::UserOutbox) {
            *message.recipient()
        } else {
            from
        };
        let is_inbox = matches!(topic, Topic::UserInbox);
        if (is_inbox && *message.recipient() != *self.user_identity.id())
            || (!is_inbox && from != *self.user_identity.id())
        {
            warn!("Handshake is not addressed to this user");
            return Ok(());
        }

        let handshake = match self.open_handshake(message.payload, &friend_id, &from) {
            Ok(handshake) => handshake,
            Err(error) => {
                warn!("{error}");
                return Ok(());
            }
        };
        match handshake.body {
            HandshakeType::FriendRequest(hello) => {
                self.on_friend_request(
                    friend_id,
                    if is_inbox {
                        from
                    } else {
                        *self.user_identity.id()
                    },
                    hello.unwrap_or_default(),
                    handshake.timestamp,
                    is_inbox,
                ).await;
                Ok(())
            }
            HandshakeType::FriendRequestAccept(session_key) => {
                self.accept_friend_handshake(
                    friend_id,
                    session_key,
                    handshake.timestamp,
                    is_inbox
                )
                .await;
                Ok(())
            }
        }
    }

    /// Inbound stages 3 and 4 for a handshake: unprotect and decode it.
    fn open_handshake(&self, payload: Value, friend_id: &Id, from: &Id) -> Result<Handshake> {
        let Value::Bytes(bytes) = payload else {
            return Err(EncodingError::new("Handshake payload is not binary").into());
        };
        let mut handshake: Handshake = serde_cbor::from_slice(&bytes).map_err(|error| {
            EncodingError::new(format!("Malformed handshake: {error}"))
        })?;
        handshake.from = Some(*from);
        let _ = friend_id;
        Ok(handshake)
    }


    async fn on_friend_request(
        &self,
        user_id: Id,
        initiator_id: Id,
        hello: String,
        timestamp: i64,
        notify: bool,
    ) {
        if timestamp < 0 {
            warn!("Ignoring friend request from {user_id} with a negative timestamp");
            return;
        }
        if let Some(contact) = self.contact(&user_id).await {
            if contact.is_blocked()
                || contact.contact_type() == ContactType::Friend
                || contact.contact_type() == ContactType::Channel
            {
                return;
            }
        }

        let created_at = Self::timestamp(timestamp);
        if self.friend_request(&user_id).await
            .is_some_and(|request| request.is_accepted() || request.created_at() >= created_at)
        {
            debug!("Ignoring duplicate or stale friend request from {user_id}");
            return;
        }

        let request = FriendRequest::new(user_id, initiator_id, Some(hello.clone()), created_at);
        if let Err(error) = self.repository.borrow_mut().put_friend_request(&request) {
            warn!("Persisting friend request failed: {error}");
        }
        self.friend_requests_cache.borrow_mut().insert(user_id, request);

        if notify {
            self.friend_request_listener
                .on_friend_request(&user_id, Some(&hello));
        }
    }

    pub(super) fn pending_friend_request(&self, user_id: &Id, initiator_id: &Id) -> Result<FriendRequest> {
        let requests = self.friend_requests_cache.borrow();
        let request = requests
            .get(user_id)
            .ok_or_else(|| ArgumentError::new(format!("No friend request found for {user_id}")))?;
        if self.contacts_cache.borrow().get(user_id).is_some_and(|contact| {
            contact.is_blocked() || contact.contact_type() == ContactType::Channel
        }) {
            return Err(ArgumentError::new(
                "Cannot accept a friend request from a blocked user or channel",
            ));
        }
        if request.initiator_id() != initiator_id {
            return Err(ArgumentError::new(
                "Friend request has an unexpected initiator",
            ));
        }
        if request.is_accepted() {
            return Err(ArgumentError::new(
                "Friend request has already been accepted",
            ));
        }
        if request.is_expired() {
            return Err(ArgumentError::new("Friend request has expired"));
        }
        Ok(request.clone())
    }

    /// Records a friend locally, including their session key.
    pub(super) async fn store_friend_contact(&self, user_id: &Id, session: &CryptoIdentity) -> Result<()> {
        if self
            .contacts_cache
            .borrow()
            .get(user_id)
            .is_some_and(|contact| contact.contact_type() == ContactType::Friend)
        {
            return Ok(());
        }
        let revision = self.contacts_revision.get();
        let mut data = ContactData::new(
            *user_id,
            ContactType::Friend as u8,
            Self::current_timestamp()?,
        );
        data.set_session_key(self.user_identity.encrypt_into(
            self.user_id(),
            session.signature_key().private_key().as_ref(),
        )?);
        data.revision = revision;
        let session_key = data.session_key().map(|key| key.to_vec());
        let mutation = ContactMutation::add(revision, data.encode_into(&self.user_identity)?);
        self.apply_contact_mutation(mutation, revision, "Friend contact revision is outdated")
            .await?;
        if let Some(contact) = self.contacts_cache.borrow().get(user_id).cloned() {
            self.persist_contact(&contact, session_key.as_deref())?;
        }
        Ok(())
    }

    /// Completes an acceptance we did not initiate: the peer said yes.
    pub(super) async fn accept_friend_handshake(
        &self,
        friend_id: Id,
        session_key: Vec<u8>,
        timestamp: i64,
        is_inbox: bool,
    ) {
        let _request = self.requests.lock().await;
        if self
            .friend_requests_cache
            .borrow()
            .get(&friend_id)
            .is_some_and(|request| request.is_accepted())
        {
            debug!("Ignoring duplicate friend request acceptance from {friend_id}");
            return;
        }
        let initiator_id = if is_inbox {
            *self.user_identity.id()
        } else {
            friend_id
        };
        let request = match self.pending_friend_request(&friend_id, &initiator_id) {
            Ok(req) => req,
            Err(_) => return,
        };
        if Self::timestamp(timestamp) < request.created_at() {
            warn!("Friend request acceptance predates the request");
            return;
        }
        let session_identity = match CryptoIdentity::try_from(session_key.as_slice()) {
            Ok(identity) => identity,
            Err(error) => {
                warn!("Invalid friend session key: {error}");
                return;
            }
        };
        if let Err(error) = self.store_friend_contact(&friend_id, &session_identity).await {
            warn!("Failed to store friend contact: {error}");
            return;
        }
        self.session_contexts
            .lock()
            .unwrap()
            .entry(friend_id)
            .or_insert_with(|| self.session_context(friend_id, session_identity, false));
        self.on_friend_request_accepted(friend_id, timestamp, is_inbox);
    }

    pub(super) fn on_friend_request_accepted(&self, user_id: Id, timestamp: i64, notify: bool) {
        if timestamp < 0 {
            warn!("Ignoring friend request acceptance from {user_id} with a negative timestamp");
            return;
        }

        let accepted_at = Self::timestamp(timestamp);
        {
            let mut requests = self.friend_requests_cache.borrow_mut();
            let Some(request) = requests.get_mut(&user_id) else {
                return;
            };
            if request.is_accepted() || request.is_expired() {
                return;
            }
            let initiator_id = if notify {
                self.user_identity.id()
            } else {
                &user_id
            };
            if request.initiator_id() != initiator_id || accepted_at < request.created_at() {
                warn!("Ignoring friend request acceptance from {user_id} that does not match the pending request");
                return;
            }
            request.accept_at(accepted_at);
        }
        if let Some(request) = self.friend_requests_cache.borrow().get(&user_id).cloned() {
            if let Err(error) = self.repository.borrow_mut().put_friend_request(&request) {
                warn!("Persisting accepted friend request failed: {error}");
            }
        }

        if self
            .contacts_cache
            .borrow()
            .get(&user_id)
            .is_some_and(|contact| contact.contact_type() == ContactType::Friend)
        {
            if notify {
                self.friend_request_listener
                    .on_friend_request_accepted(&user_id);
            }
            return;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        let mut contact = Box::new(PhotonContact::new(user_id, ContactType::Friend, now_ms)) as Contact;
        contact.set_revision(self.contacts_revision.get());
        self.contacts_cache.borrow_mut().insert(user_id, contact.clone());
        if notify {
            self.friend_request_listener
                .on_friend_request_accepted(&user_id);
        }
        self.contact_listener.on_contact_added(&contact);
    }

    fn process_incoming_content_message(
        &self,
        mut message: PhotonMessage,
        from: Id
    ) {
       let contact_id = if *message.recipient() != *self.user_id() {
            *message.recipient()
        } else {
            from
        };
        if self
            .contacts_cache
            .borrow()
            .get(&contact_id)
            .is_some_and(|contact| contact.is_blocked())
        {
            warn!("Message is from a blocked contact");
            return;
        }
        if *message.recipient() != *self.user_id()
            && !self
                .contacts_cache
                .borrow()
                .get(&contact_id)
                .is_some_and(|contact| contact.contact_type() == ContactType::Channel)
        {
            warn!("Content message has an unexpected recipient");
            return;
        }
        let content_bytes = message.payload_bytes.clone();
        let content = match PhotonContent::parse(&content_bytes) {
            Ok(content) => content,
            Err(error) => {
                warn!("Failed to parse message content: {error}");
                return;
            }
        };
        message.set_from(from);
        let now = now_millis();
        message.set_received_at(now);
        
        message.payload_bytes = content_bytes;
        message.content = Some(content);
        message.set_converstation_id(contact_id);
        let created_at = match i64::try_from(message.created_at_u64()) {
            Ok(timestamp) => timestamp,
            Err(_) => {
                warn!("Message timestamp is out of range");
                return;
            }
        };
        let conversation_contact = self
            .contacts_cache
            .borrow()
            .get(&contact_id)
            .cloned()
            .unwrap_or_else(|| Box::new(PhotonContact::new(contact_id, ContactType::Auto, created_at)) as Contact);
        if let Err(error) = self.repository.borrow_mut().put_message(&mut message) {
            warn!("Persisting inbound message failed: {error}");
        }
        let boxed_message: Message = Box::new(message.clone());
        if let Err(error) = update_conversation(
            &self.conversations_cache.shared(),
            conversation_contact,
            boxed_message.clone(),
        ) {
            warn!("Failed to update conversation: {error}");
        }
        self.message_listener.on_message(&boxed_message);
    }

    fn process_outgoing_content_message(&self, mut message: PhotonMessage, from: Id) {

        let contact_id = *message.recipient();
        let content_bytes = message.payload_bytes.clone();
        let content = match PhotonContent::parse(&content_bytes) {
            Ok(content) => content,
            Err(error) => {
                warn!("Failed to parse message content: {error}");
                return;
            }
        };
        message.set_from(from);
        message.set_sent(now_millis());
        message.payload_bytes = content_bytes;
        message.content = Some(content);
        message.set_converstation_id(contact_id);
        let created_at = match i64::try_from(message.created_at_u64()) {
            Ok(timestamp) => timestamp,
            Err(_) => {
                warn!("Message timestamp is out of range");
                return;
            }
        };
        let originated_here = match Self::message_id(self.device_id(), created_at) {
            Ok(id) => MessageOps::id(&message) == &id,
            Err(error) => {
                warn!("Failed to derive message id: {error}");
                return;
            }
        };
        if originated_here {
            self.message_listener.on_sent(&(Box::new(message) as Message));
            return;
        }
        let conversation_contact = self
            .contacts_cache
            .borrow()
            .get(&contact_id)
            .cloned()
            .unwrap_or_else(|| Box::new(PhotonContact::new(contact_id, ContactType::Auto, created_at)) as Contact);
        if let Err(error) = self.repository.borrow_mut().put_message(&mut message) {
            warn!("Persisting outbound message failed: {error}");
        }
        let boxed_message: Message = Box::new(message.clone());
        if let Err(error) = update_conversation(
            &self.conversations_cache.shared(),
            conversation_contact,
            boxed_message.clone(),
        ) {
            warn!("Failed to update conversation: {error}");
        }
        self.message_listener.on_sent(&boxed_message);
    }

    /// Inbound stages 3 and 4 for a content message.
    ///
    /// Returns the plaintext CBOR document together with its headers and body.
    /// Our own outbox copies are sealed for the session, everything else by
    /// the sender with the session key.
    pub(super) fn open_content(
        &self,
        contact_id: &Id,
        outbox: bool,
        from: &Id,
        payload: &Value,
    ) -> Result<(Vec<u8>, PhotonContent)> {
        let encrypted = match payload {
            Value::Bytes(bytes) => Ok(bytes),
            _ => Err(EncodingError::new(format!("Content payload is not binary"))),
        }?;

        let content_bytes = {
            let mut sessions = self.session_contexts.lock().unwrap();
            let session = sessions
                .get_mut(contact_id)
                .ok_or_else(|| StateError::new(format!("No friend session for {contact_id}")))?;
            if outbox {
                session.tx_crypto_context()?.decrypt_into(&encrypted)?
            } else {
                session
                    .rx_crypto_context_for(from)?
                    .decrypt_into(&encrypted)?
            }
        };

        let content = PhotonContent::parse(&encrypted)
            .map_err(|error| EncodingError::new(format!("Malformed content: {error}")))?;

        if !matches!(content.format(), ContentFormat::Text | ContentFormat::Binary) {
            return Err(EncodingError::new("Unsupported message content body"));
        }
        Ok((content_bytes, content))
    }

    pub(super) fn timestamp(timestamp: i64) -> SystemTime {
        if timestamp < 0 {
            return SystemTime::now();
        }
        UNIX_EPOCH + Duration::from_millis(timestamp as u64)
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

        // 2. Build the request.
        let request = RPCRequest::contact_sync(0, revision);

        // 3. Send it to the home peer and 4. decode the delta it reports.
        let result = self.send_control(self.peer_id(), request).await?;
        let ResponseParams::ContactSync(sync) = result else {
            return Err(EncodingError::new("RPC result is not a contact sync").into());
        };

        // 5. Apply the delta to the local state.
        self.apply_contact_sync(&sync).await
    }

    /// Sends a contact mutation document and adopts the revision it reports.
    ///
    /// `minimum` is the revision the caller needs to see: plain mutations only
    /// require the current one, while `update_contact` insists on the revision
    /// it computed so that a concurrent change cannot silently win.
    pub(super) async fn commit_contact_mutation(
        &self,
        mutation: ContactMutation,
        minimum: i32,
        outdated: &str,
    ) -> Result<i32> {
        let request = RPCRequest::contact_mutate(0, mutation);
        let result = self.send_control(self.peer_id(), request).await?;
        let ResponseParams::ContactMutate(revision) = result else {
            return Err(EncodingError::new("RPC result is not a contact revision").into());
        };
        if revision < minimum || revision < self.contacts_revision.get() {
            return Err(StateError::new(outdated));
        }
        self.contacts_revision.set(revision);
        Ok(revision)
    }

    /// Encodes and sends a [`ContactMutation`].
    pub(super) async fn apply_contact_mutation(
        &self,
        mutation: ContactMutation,
        minimum: i32,
        outdated: &str,
    ) -> Result<i32> {
        self.commit_contact_mutation(mutation, minimum, outdated)
            .await
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
