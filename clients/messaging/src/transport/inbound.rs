use super::prelude::*;
use super::outbound::update_conversation;

use crate::internal::notification::{
    Body,
    Notification,
};

impl RpcServer {
    pub(super) async fn dispatch(
        &self,
        topic: Topic,
        message: PhotonMessage
    ) -> Result<()> {
        if message.version != MESSAGE_VERSION {
            return Err(ProtocolError::new(
                message.version as i32,"Unsupported message version",
            ));
        }
        let from = message.from().copied().ok_or_else(
            || EncodingError::new("Incoming message has no sender")
        )?;

        match (topic, message.message_type) {
            (Topic::DeviceInbox, mt) if mt == MessageType::Control as u8
                && (message.recipient == *self.user_id()
                    || message.recipient == *self.device_id()) =>
            {
                self.on_device_rpc(message, &from).map_err(|e|e.into())
            }
            (Topic::DeviceInbox, _) => {
                return Err(EncodingError::new("Invalid device RPC response").into());
            }
            (Topic::UserInbox, mt) if mt == MessageType::State as u8 => {
                self.on_state_message(message, &from).await.map_err(|e|e.into())
            }
            (_, mt) if mt == MessageType::State as u8 => {
                return Err(EncodingError::new("Invalid home-peer notification").into());
            }
            (_, mt) if mt == MessageType::Handshake as u8 => {
                self.on_handshake(topic, message, from).await.map_err(|e|e.into())
            }
            (_, mt) if mt == MessageType::Content as u8 => {
                self.on_content_message(topic, message, from).map_err(|e|e.into())
            }
            _ => return Err(EncodingError::new("Unsupported user message type").into()),
        }
    }

    /// Handles a reply to a control RPC.
    ///
    /// Replies from the service are sealed with the device key only.
    /// Replies relayed on behalf of a channel peer carry a second layer that only this
    /// user can open, so they are decrypted twice.
    fn on_device_rpc(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        let payload = match message.payload {
            Value::Bytes(bytes) => Ok(bytes),
            _ => Err(EncodingError::new(format!("Rpc response payload is not binary"))),
        }?;
        let payload = if from == self.peer_id() {
            payload
        } else {
            self.user_identity.decrypt_into(from, &payload).map_err(|error| {
                AuthenticationError::new(format!("Decrypting Rpc response failed: {error}"))
            })?
        };
        self.on_rpc_response(&payload, from)
    }

    fn on_rpc_response(&self, payload: &[u8], from: &Id) -> Result<()> {
        self.pending_calls
            .respond(
                serde_cbor::from_slice(payload).map_err(|error|
                    EncodingError::new(format!("Malformed RPC response: {error}"))
                )?,
                from,
            )
            .map_err(|e| e.into())
    }

    async fn on_state_message(&self, message: PhotonMessage, from: &Id) -> Result<()> {
        let payload = match message.payload {
            Value::Bytes(bytes) => Ok(bytes),
            _ => Err(EncodingError::new(format!("Notification payload is not binary"))),
        }?;

        let home = from == self.peer_id() && message.recipient == *self.user_id();
        let recipient = message.recipient;
        let payload = if home {
            payload
        } else if &recipient == self.user_id() {
            self.user_identity.decrypt_into(from, &payload).map_err(|error| {
                AuthenticationError::new(format!("Decrypting notification error: {error}"))
            })?
        } else if &recipient == from {
            let mut contexts = self.session_contexts.lock().unwrap();
            let Some(session_context) = contexts.get_mut(from) else {
                return Err(StateError::new("Channel notification session mutex is missing"));
            };
            session_context
                .rx_crypto_context()?
                .decrypt_into(&payload)
                .map_err(|error| {
                    AuthenticationError::new(format!(
                        "Decrypting channel notification envelope failed: {error}"
                    ))
                })?
        } else {
            return Err(EncodingError::new("Invalid channel notification recipient").into());
        };

        self.on_notification(&payload, from).map_err(|e| e.into())
    }

    fn on_notification(&self, payload: &[u8], from: &Id) -> Result<()> {
        let notification: Notification = serde_cbor::from_slice(payload).map_err(|error|
            EncodingError::new(format!("Malformed notification: {error}"))
        )?;

        let timestamp = notification.timestamp() as i64;
        if notification.source() == self.user_id()
            && notification.is_channel_notification()
            && notification.id() == &RpcServer::message_id(self.device_id(), timestamp)?
        {
            return Ok(());
        }

        if self.peer_id() == from {
            self.on_homepeer_notification(&notification)
        } else {
            self.on_channel_notification(&notification, from)
        }
    }

    fn on_homepeer_notification(&self, notification: &Notification) -> Result<()> {
        match notification.body() {
            Body::SessionNew(session) => {
                self.session_listener.on_new_session(session);
                Ok(())
            }
            Body::ContactSync(sync) => {
                self.apply_contact_sync(sync)?;
                if self.connected.get() && !self.stopping.get() && !self.ready.replace(true) {
                    info!("Messaging session is ready");
                    self.notify_connection(|l| l.on_ready());
                }
                Ok(())
            }
            Body::ChannelCreate(info) => {
                self.upsert_channel(info, self.peer_id(), true)
            }
            body => Err(EncodingError::new(format!(
                "Unsupported home-peer notification: {body:?}"
            )))
        }
    }

    fn on_channel_notification(
        &self,
        notification: &Notification,
        sender: &Id,
    ) -> Result<()> {
        match notification.body() {
            Body::ChannelCreate(info) => {
                self.upsert_channel(info, sender, true)
            }
            Body::ChannelJoin(info) => {
                self.upsert_channel(info, sender, false)
            }
            Body::ChannelDelete | Body::ChannelLeave => {
                let Some(channel) = self.channels_cache.borrow_mut().remove(sender) else {
                    return Ok(());
                };
                self.contacts_cache.borrow_mut().remove(sender);
                self.session_contexts.lock().unwrap().remove(sender);
                if matches!(notification.body(), Body::ChannelDelete) {
                    self.channel_listener.on_channel_deleted(&channel);
                } else {
                    self.channel_listener.on_left_channel(&channel);
                }
                Ok(())
            }
            Body::ChannelSessionKeyRotate(encrypted_key) => {
                let mut channel = self.channel_snapshot(sender)?;
                if notification.source() != channel.owner() {
                    return Err(EncodingError::new(
                        "Channel session key rotation has an unexpected source",
                    ).into());
                }
                let session = self
                    .session_contexts
                    .lock()
                    .unwrap()
                    .get(sender)
                    .cloned()
                    .ok_or_else(|| StateError::new("Channel session key is missing"))?;
                let key = session
                    .decrypt_into(&notification.source(), encrypted_key)
                    .map_err(|error| {
                        AuthenticationError::new(format!(
                            "Decrypting rotated channel session key failed: {error}"
                        ))
                    })?;
                let session = CryptoIdentity::try_from(key.as_slice()).map_err(|error| {
                    AuthenticationError::new(format!(
                        "Invalid rotated channel session key: {error}"
                    ))
                })?;
                channel.set_session_id(*session.id());
                self.session_contexts
                    .lock()
                    .unwrap()
                    .insert(*sender, self.session_context(*sender, session, true));
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_session_key_rotated(&channel);
                Ok(())
            }
            Body::ChannelOwnershipTransfer(new_owner) => {
                let mut channel = self.channel_snapshot(sender)?;
                let old_owner = *channel.owner();
                channel.set_owner(*new_owner);
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_ownership_transferred(&channel, &old_owner, new_owner);
                Ok(())
            }
            Body::ChannelInfoUpdate(changes) => {
                let mut channel = self.channel_snapshot(sender)?;
                if let Some(permission) = changes.get_permission() {
                    channel.set_permission(permission);
                }
                if let Some(name) = changes.get_name() {
                    channel.set_name(name.map(ToString::to_string));
                }
                if let Some(notice) = changes.get_notice() {
                    channel.set_notice(notice.map(ToString::to_string));
                }
                if let Some(announced) = changes.get_announced() {
                    channel.set_announced(announced);
                }
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener.on_channel_updated(&channel);
                Ok(())
            }
            Body::ChannelMembersRoleUpdate(update) => {
                let mut channel = self.channel_snapshot(sender)?;
                let changed = channel.update_member_roles(update.member_ids(), update.role());
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_members_role_updated(&channel, &changed);
                Ok(())
            }
            Body::ChannelMembersBan(ids) => {
                let mut channel = self.channel_snapshot(sender)?;
                let changed = channel.update_member_roles(ids, Role::Banned);
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_members_banned(&channel, &changed);
                Ok(())
            }
            Body::ChannelMembersUnban(ids) => {
                let mut channel = self.channel_snapshot(sender)?;
                let changed = channel.update_member_roles(ids, Role::Member);
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_members_unbanned(&channel, &changed);
                Ok(())
            }
            Body::ChannelMembersRemove(ids) => {
                let mut channel = self.channel_snapshot(sender)?;
                let removed = channel.remove_members(ids);
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_members_removed(&channel, &removed);
                Ok(())
            }
            Body::ChannelMemberJoin(member) => {
                member.joined()
                    .duration_since(UNIX_EPOCH)
                    .map(|_| ())
                    .map_err(|_| EncodingError::new("Channel member timestamp is out of range"))?;

                let mut channel = self.channel_snapshot(sender)?;
                channel.upsert_member(*member.id(), member.role(), member.joined());
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                self.channel_listener
                    .on_channel_member_joined(&channel, &(Box::new(member.clone()) as ChannelMember));
                Ok(())
            }
            Body::ChannelMemberLeave(id) => {
                let mut channel = self.channel_snapshot(sender)?;
                let member = channel.members().into_iter().find(|member| member.id() == id);
                channel.remove_members(&[*id]);
                self.channels_cache.borrow_mut().insert(*sender, channel.clone());
                if let Some(member) = member {
                    self.channel_listener
                        .on_channel_member_left(&channel, &member);
                }
                Ok(())
            }
            body => Err(EncodingError::new(format!(
                "Unsupported channel notification: {body:?}"
            ))),
        }
    }

    fn upsert_channel(&self, info: &ChannelInfo, sender: &Id, created: bool) -> Result<()> {
        let (channel, session) = self.channel_from_info(info)?;
        let id = *channel.id();
        if !created && id != *sender {
            return Err(EncodingError::new(
                "Channel notification has an unexpected source",
            ));
        }
        let known = self.channels_cache.borrow().contains_key(&id);
        self.session_contexts.lock().unwrap().insert(id, session);
        self.contacts_cache
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| channel.contact_box());
        self.channels_cache
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| channel.clone());
        if known {
            return Ok(());
        }
        if created {
            self.channel_listener.on_channel_created(&channel);
        } else {
            self.channel_listener.on_joined_channel(&channel);
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
                let info: ChannelContactInfo = decode_cbor_bytes(&plaintext, "channel contact")?;
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

    fn apply_contact_sync(&self, sync: &ContactSync) -> Result<()> {
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

    async fn on_handshake(&self, topic: Topic, message: PhotonMessage, from: Id) -> Result<()> {
        let timestamp = i64::try_from(message.created_at)
            .map_err(|_| EncodingError::new("Message timestamp is out of range"))?;
        let originated_here = message.id == Self::message_id(self.device_identity.id(), timestamp)?;
        if matches!(topic, Topic::UserOutbox) && originated_here {
            return Ok(());
        }
        let friend_id = if matches!(topic, Topic::UserOutbox) {
            message.recipient
        } else {
            from
        };
        let is_inbox = matches!(topic, Topic::UserInbox);
        if (is_inbox && message.recipient != *self.user_identity.id())
            || (!is_inbox && from != *self.user_identity.id())
        {
            return Err(EncodingError::new(
                "Handshake is not addressed to this user",
            ));
        }

        let handshake = self.open_handshake(message.payload, &friend_id, &from)?;

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
                );
                Ok(())
            }
            HandshakeType::FriendRequestAccept(session_key) => self
                .accept_friend_handshake(friend_id, session_key, handshake.timestamp, is_inbox)
                .await
                .map_err(|e| e.into()),
        }
    }

    /// Inbound stages 3 and 4 for a handshake: unprotect and decode it.
    fn open_handshake(&self, payload: Value, friend_id: &Id, from: &Id) -> Result<Handshake> {
        let encrypted = binary_payload(payload, "Handshake")?;
        let bytes = decrypt_payload(&self.user_identity, friend_id, &encrypted, "handshake")?;
        let mut handshake: Handshake = decode_cbor_bytes(&bytes, "handshake")?;
        handshake.from = Some(*from);
        Ok(handshake)
    }

    pub(super) fn on_friend_request(
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
        if let Some(contact) = self.contacts_cache.borrow().get(&user_id) {
            if contact.is_blocked()
                || contact.contact_type() == ContactType::Friend
                || contact.contact_type() == ContactType::Channel
            {
                return;
            }
        }
        let created_at = Self::timestamp(timestamp);
        if self
            .friend_requests_cache
            .borrow()
            .get(&user_id)
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
    ) -> Result<()> {
        let _request = self.requests.lock().await;
        if self
            .friend_requests_cache
            .borrow()
            .get(&friend_id)
            .is_some_and(|request| request.is_accepted())
        {
            debug!("Ignoring duplicate friend request acceptance from {friend_id}");
            return Ok(());
        }
        let initiator_id = if is_inbox {
            *self.user_identity.id()
        } else {
            friend_id
        };
        let request = self.pending_friend_request(&friend_id, &initiator_id)?;
        if Self::timestamp(timestamp) < request.created_at() {
            return Err(EncodingError::new(
                "Friend request acceptance predates the request",
            ));
        }
        let session_identity =
            CryptoIdentity::try_from(session_key.as_slice()).map_err(|error| {
                AuthenticationError::new(format!("Invalid friend session key: {error}"))
            })?;
        self.store_friend_contact(&friend_id, &session_identity)
            .await?;
        self.session_contexts
            .lock()
            .unwrap()
            .entry(friend_id)
            .or_insert_with(|| self.session_context(friend_id, session_identity, false));
        self.on_friend_request_accepted(friend_id, timestamp, is_inbox);
        Ok(())
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

    fn on_content_message(
        &self,
        topic: Topic,
        mut message: PhotonMessage,
        from: Id,
    ) -> Result<()> {
        let outbox = matches!(topic, Topic::UserOutbox);
        if outbox && from != *self.user_id() {
            return Err(EncodingError::new(
                "Outbox message has an unexpected sender",
            ));
        }
        let contact_id = if outbox || message.recipient != *self.user_id() {
            message.recipient
        } else {
            from
        };
        if self
            .contacts_cache
            .borrow()
            .get(&contact_id)
            .is_some_and(|contact| contact.is_blocked())
        {
            return Err(StateError::new("Message is from a blocked contact"));
        }
        if !outbox
            && message.recipient != *self.user_id()
            && !self
                .contacts_cache
                .borrow()
                .get(&contact_id)
                .is_some_and(|contact| contact.contact_type() == ContactType::Channel)
        {
            return Err(EncodingError::new(
                "Content message has an unexpected recipient",
            ));
        }
        let (content_bytes, content) =
            self.open_content(&contact_id, outbox, &from, &message.payload)?;
        message.from = Some(from);
        message.received_at = if outbox {
            None
        } else {
            Some(SystemTime::now())
        };
        message.sent_at = if outbox {
            Some(SystemTime::now())
        } else {
            None
        };
        message.payload_bytes = content_bytes;
        message.content = Some(content);
        message.conversation_id = Some(contact_id);
        let created_at = i64::try_from(message.created_at)
            .map_err(|_| EncodingError::new("Message timestamp is out of range"))?;
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
        update_conversation(&self.conversations_cache, conversation_contact, boxed_message.clone())?;
        if outbox {
            if message.id != Self::message_id(self.device_id(), created_at)? {
                self.message_listener.on_sent(&boxed_message);
            }
        } else {
            self.message_listener.on_message(&boxed_message);
        }
        Ok(())
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
        let encrypted = binary_payload(payload.clone(), "Content")?;
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
        let wire: PhotonContent = decode_cbor_bytes(&content_bytes, "message content")?;
        let content = PhotonContent::new(wire.headers, wire.format, wire.body)?;
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
        Id::try_from_bytes(digest.finalize().as_slice())
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
        self.apply_contact_sync(&sync)
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

fn binary_payload(payload: Value, what: &str) -> Result<Vec<u8>> {
    match payload {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(EncodingError::new(format!("{what} payload is not binary"))),
    }
}

fn decrypt_payload(
    identity: &CryptoIdentity,
    sender: &Id,
    cipher: &[u8],
    what: &str,
) -> Result<Vec<u8>> {
    identity.decrypt_into(sender, cipher).map_err(|error| {
        AuthenticationError::new(format!("Decrypting {what} failed: {error}")).into()
    })
}
