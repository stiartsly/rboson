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
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime},
};
use futures::{
    stream::FuturesUnordered,
    FutureExt,
};
use log::{debug, warn};
use serde::Deserialize;

use boson_api::{
    Result,
    errors::StateError,
    CryptoIdentity,
    Id,
    Identity,
};

use crate::{
    channel::Channel,
    contact::Contact,
    internal::session_context::SessionContext,
};

pub(super) const MESSAGE_VERSION: u8 = 2;
pub(super) const HANDSHAKE_MESSAGE: u8 = 0;

pub(super) const RPC_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) const FIELD_SESSION_KEY: &str = "sk";
pub(super) const FIELD_OWNER: &str = "o";
pub(super) const FIELD_PERMISSION: &str = "p";
pub(super) const FIELD_NOTICE: &str = "nt";
pub(super) const FIELD_ANNOUNCED: &str = "a";

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
    pub(super) contact: Contact,
    pub(super) session: Option<CryptoIdentity>,
    pub(super) channel: Option<Channel>,
}

pub(super) struct ContactState {
    pub(super) contacts: HashMap<Id, Contact>,
    pub(super) sessions: HashMap<Id, SessionContext>,
    pub(super) channels: HashMap<Id, Channel>,
    pub(super) revision: i32,
}

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

    fn persist_contact(&self, contact: &Contact, session_key: Option<&[u8]>) -> Result<()> {
        self.repository
            .borrow_mut()
            .put_contact(self.contacts_revision.get(), contact, session_key)
    }

    fn persist_channel(&self, channel: &Channel) -> Result<()> {
        let key = self.persisted_session_key(channel.id())?;
        self.repository
            .borrow_mut()
            .put_channel(self.contacts_revision.get(), channel, &key)
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

    fn origin_timestamp(clock: &std::sync::Mutex<i64>) -> i64 {
        let now = boson_api::as_ms!(SystemTime::now()) as i64;
        let mut prev = clock.lock().unwrap();
        let next = prev.checked_add(1).unwrap();
        let timestamp = now.max(next);
        *prev = timestamp;
        timestamp
    }

    fn fail_pending_rpc(&self, reason: &str) {
        self.pending_calls.fail_all(reason);
    }

    /// Reclaims expired local records; run periodically by the verticle timer.
    pub(crate) fn maintenance(&self) {
        let mut requests = self.friend_requests_cache.borrow_mut();
        let before = requests.len();
        requests.retain(|_, request| !request.is_expired());
        let removed = before - requests.len();
        if removed > 0 {
            debug!("Reclaimed {removed} expired friend request(s)");
        }
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

    fn close_transport(&self, reason: &str) {
        self.running.set(false);
        self.fail_pending_rpc(reason);
        self.close();
        self.reconnect_at.set(None);
        self.ready.set(false);
        if self.connected.replace(false) {
            self.notify_connection(|l| l.on_disconnected());
        }
    }

    pub(crate) fn user_id(&self) -> &Id {
        self.options.user_id()
    }

    pub(crate) fn device_id(&self) -> &Id {
        self.options.device_id()
    }

    pub(crate) fn peer_id(&self) -> &Id {
        self.options.peer_id()
    }

}

mod prelude;
mod rpc_server;
mod transport;
mod dispatch;
mod inbound;
mod outbound;
mod mqtt;
pub(crate) use rpc_server::RpcServer;
