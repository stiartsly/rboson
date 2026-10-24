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

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

use boson_api::CryptoContext;
use boson_api::errors::ArgumentError;
use boson_api::{CryptoIdentity, Id, Identity, Result};

/// Distinguishes one-to-one sessions from channel sessions, which keep a
/// separate crypto context per sending member.
enum ContextKind {
    User,
    Channel(HashMap<Id, CryptoContext>),
}

/// Session key material and cached crypto contexts for a single contact.
pub(crate) struct SessionContext {
    contact_id: Id,
    user_identity: Arc<CryptoIdentity>,
    session_identity: Arc<CryptoIdentity>,

    rx_crypto_context: Option<CryptoContext>,
    tx_crypto_context: Option<CryptoContext>,
    kind: ContextKind,
}

impl SessionContext {
    fn new(
        contact_id: Id,
        user_identity: Arc<CryptoIdentity>,
        session_identity: Arc<CryptoIdentity>,
        kind: ContextKind,
    ) -> Self {
        Self {
            contact_id,
            user_identity,
            session_identity,
            rx_crypto_context: None,
            tx_crypto_context: None,
            kind,
        }
    }

    pub(crate) fn for_user(
        contact_id: Id,
        user_identity: Arc<CryptoIdentity>,
        session_identity: Arc<CryptoIdentity>,
    ) -> Self {
        Self::new(contact_id, user_identity, session_identity, ContextKind::User)
    }

    pub(crate) fn for_channel(
        contact_id: Id,
        user_identity: Arc<CryptoIdentity>,
        session_identity: Arc<CryptoIdentity>,
    ) -> Self {
        Self::new(
            contact_id,
            user_identity,
            session_identity,
            ContextKind::Channel(HashMap::new()),
        )
    }

    pub(crate) fn session_id(&self) -> &Id {
        self.session_identity.id()
    }

    pub(crate) fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.session_identity.sign_into(data)
    }

    pub(crate) fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        self.session_identity.verify(data, signature)
    }

    pub(crate) fn encrypt(&self, recipient: &Id, data: &[u8]) -> Result<Vec<u8>> {
        self.session_identity.encrypt_into(recipient, data)
    }

    pub(crate) fn decrypt(&self, sender: &Id, data: &[u8]) -> Result<Vec<u8>> {
        self.session_identity.decrypt_into(sender, data)
    }

    pub(crate) fn create_crypto_context(&self, id: &Id) -> Result<CryptoContext> {
        self.session_identity.create_crypto_context(id)
    }

    /// Uses the user's private key and the session's public key, as in the wire protocol.
    pub(crate) fn tx_crypto_context(&mut self) -> Result<&mut CryptoContext> {
        match self.tx_crypto_context {
            Some(ref mut context) => Ok(context),
            None => {
                let context = self
                    .user_identity
                    .create_crypto_context(self.session_identity.id())?;
                Ok(self.tx_crypto_context.insert(context))
            }
        }
    }

    pub(crate) fn rx_crypto_context(&mut self) -> Result<&mut CryptoContext> {
        match self.rx_crypto_context {
            Some(ref mut context) => Ok(context),
            None => {
                let context = self.create_crypto_context(&self.contact_id)?;
                Ok(self.rx_crypto_context.insert(context))
            }
        }
    }

    /// Returns the receive context for `sender`, caching per member for channels.
    pub(crate) fn rx_crypto_context_for(&mut self, sender: &Id) -> Result<&mut CryptoContext> {
        if sender == &self.contact_id {
            return self.rx_crypto_context();
        }

        match &mut self.kind {
            ContextKind::User => Err(ArgumentError::new(
                "Invalid sender: expected the session contact",
            )),
            ContextKind::Channel(contexts) => match contexts.entry(*sender) {
                Entry::Occupied(entry) => Ok(entry.into_mut()),
                Entry::Vacant(entry) => {
                    let context = self.session_identity.create_crypto_context(sender)?;
                    Ok(entry.insert(context))
                }
            },
        }
    }
}

#[cfg(test)]
#[path = "../unitests/test_session_context.rs"]
mod test_session_context;
