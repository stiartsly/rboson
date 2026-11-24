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
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::{Duration, SystemTime},
};
use futures::{
    stream::FuturesUnordered,
    FutureExt,
};
use log::warn;

use boson_api::{
    Result,
    errors::StateError,
    CryptoIdentity,
    Id,
    Identity,
};

use crate::{
    ContactHandler,
};
use crate::internal::{
    session_context::SessionContext,
    PhotonChannel,
    PhotonContact,
    PhotonConversation,
    PhotonFriendRequest,
};

pub(super) const MESSAGE_VERSION: u8 = 2;

pub(super) const RPC_TIMEOUT: Duration = Duration::from_secs(15);

impl RpcServer {
    fn persisted_session_key(&self, contact_id: &Id) -> Result<Vec<u8>> {
        let session = self
            .session_contexts
            .lock()
            .unwrap()
            .get(contact_id)
            .cloned()
            .ok_or_else(|| StateError::new("Contact session key is missing"))?;
        self.user_identity
            .encrypt_into(self.user_id(), session.private_key())
    }

    fn session_context(
        &self,
        contact_id: Id,
        session: CryptoIdentity,
        channel: bool,
    ) -> SessionContext {
        let user_identity = Arc::new(self.user_identity.clone());
        let session_identity = Arc::new(session);
        if channel {
            SessionContext::for_channel(contact_id, user_identity, session_identity)
        } else {
            SessionContext::for_user(contact_id, user_identity, session_identity)
        }
    }

    fn origin_timestamp(&self) -> i64 {
        let now = boson_api::as_ms!(SystemTime::now()) as i64;
        let prev = self.origin_timestamp.load(Ordering::SeqCst);
        let next = prev.checked_add(1).unwrap();
        let timestamp = now.max(next);
        let _ = self.origin_timestamp.compare_exchange(
            prev,
            timestamp,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );

        timestamp
    }

    /// Queues a contact resync if one is due and none is already running.
    pub(crate) fn schedule_contact_sync(
        self: &Rc<Self>,
        pending: &mut FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    ) {
        if self.is_running()
            && self.is_ready()
            && !self.sync_running.get()
            && self.contact_sync_needed.replace(false)
        {
            self.sync_running.set(true);
            let session = self.clone();
            pending.push(
                async move {
                    if let Err(error) = session.sync_contacts().await {
                        warn!("Contact resynchronization failed: {error}");
                    }
                    session.sync_running.set(false);
                }
                .boxed_local(),
            );
        }
    }

    fn close_transport(&self) {
        self.running.set(false);
        self.pending_calls.clear("TODO");
        self.close();
        self.reconnect_at.set(None);
        self.ready.set(false);
        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
    }

    pub(crate) fn user_id(&self) -> &Id {
        self.user_identity.id()
    }

    pub(crate) fn device_id(&self) -> &Id {
        self.device_identity.id()
    }

    pub(crate) fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

    pub(crate) fn session_context_for(&self, from: &Id) -> Option<SessionContext> {
        self.session_contexts.lock().unwrap().get(from).cloned()
    }

    pub(crate) async fn put_contact(&self, contact: &PhotonContact) -> Result<()> {
        let session_key = self.persisted_session_key(contact.id()).ok();
        let revision = self.contacts_revision.get();
        self.repository
            .borrow_mut()
            .put_contact(revision, contact, session_key.as_deref())?;
        Ok(())
    }

    pub(crate) fn get_channel(&self, id: &Id) -> Option<PhotonChannel> {
        let mut channel = self.repository.borrow_mut().load_channel(id).ok().flatten()?;
        if let Some(session) = self.session_contexts.lock().unwrap().get(id) {
            channel.set_session_id(*session.session_id());
        }
        Some(channel)
    }

    pub(crate) fn put_channel(&self, channel: &PhotonChannel) -> Result<()> {
        let _ = self.persisted_session_key(channel.id())?;
        self.repository.borrow_mut().put_channel(channel)
    }

    pub(crate) async fn remove_channel(&self, channel_id: &Id) {
        if let Err(error) = self.repository.borrow_mut().remove_channel(channel_id) {
            warn!("Failed to remove channel {channel_id}: {error}");
        }
        self.session_contexts.lock().unwrap().remove(channel_id);
    }

    pub(crate) async fn remove_contact(&self, contact_id: &Id) {
        let revision = self.contacts_revision.get();
        if let Err(error) = self.repository
            .borrow_mut()
            .remove_contact(revision, contact_id)
        {
            warn!("Failed to remove contact {contact_id}: {error}");
        }
        self.repository.borrow_mut().remove_channel(contact_id).ok();
        self.session_contexts.lock().unwrap().remove(contact_id);
        self.blocked_contacts.lock().unwrap().remove(contact_id);
    }

    pub(crate) async fn remove_channels(&self) {
        if let Err(error) = self.repository.borrow_mut().remove_channels() {
            warn!("Failed to remove channels: {error}");
        }
    }

    pub(crate) async fn conversation(&self, id: &Id) -> Option<PhotonConversation> {
        self.repository
            .borrow_mut()
            .conversation(id)
            .ok()
            .flatten()
    }

    pub(crate) async fn conversations(&self) -> Vec<PhotonConversation> {
        self.repository
            .borrow_mut()
            .load_conversations()
            .unwrap_or_default()
    }

    pub(crate) async fn put_conversation(&self, conversation: PhotonConversation) -> Result<()> {
        self.repository.borrow_mut().put_conversation(conversation)
    }

    pub(crate) async fn clear_conversations(&self, id: &Id) -> Result<()> {
        self.repository
            .borrow_mut()
            .remove_messages_in_conversation(id)
            .map(|_| ())
    }

    pub(crate) fn put_friend_request(&self, request: &PhotonFriendRequest) -> Result<()> {
        self.repository
            .borrow_mut()
            .put_friend_request(request)
    }
}

mod rpc_server;
mod transport;
mod dispatch;
mod inbound;
mod outbound;
mod mqtt;
pub(crate) use rpc_server::RpcServer;

#[cfg(test)]
#[path = "../unitests/test_transport.rs"]
mod test_transport;
