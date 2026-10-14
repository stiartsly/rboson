use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use crate::{
    Id,
    errors::ArgumentError,
    Result,
    cryptobox,
    Identity,
    CryptoIdentity,
    core::CryptoContext,
};

enum ContextKind {
    User,
    Channel(HashMap<Id, CryptoContext>),
}

pub(crate) struct SessionContext {
    contact_id: Id,
    user_identity: Arc<CryptoIdentity>,
    session_identity: Arc<CryptoIdentity>,

    rx_crypto_context: Option<CryptoContext>,
    tx_crypto_context: Option<CryptoContext>,
    kind: ContextKind,
}

impl SessionContext {
    pub(crate) fn for_user(
        contact_id: Id,
        user_identity: Arc<CryptoIdentity>,
        session_identity: Arc<CryptoIdentity>,
    ) -> Self {
        Self {
            contact_id,
            user_identity,
            session_identity,
            rx_crypto_context: None,
            tx_crypto_context: None,
            kind: ContextKind::User,
        }
    }

    pub(crate) fn for_channel(
        contact_id: Id,
        user_identity: Arc<CryptoIdentity>,
        session_identity: Arc<CryptoIdentity>,
    ) -> Self {
        Self {
            contact_id,
            user_identity,
            session_identity,
            rx_crypto_context: None,
            tx_crypto_context: None,
            kind: ContextKind::Channel(HashMap::new()),
        }
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
            _ => {
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
            _ => {
                let context = self.create_crypto_context(&self.contact_id)?;
                Ok(self.rx_crypto_context.insert(context))
            }
        }
    }

    pub(crate) fn rx_crypto_context_for(&mut self, sender: &Id) -> Result<&mut CryptoContext> {
        if sender == &self.contact_id {
            return self.rx_crypto_context();
        }

        match &mut self.kind {
            ContextKind::User => Err(ArgumentError::new(
                "Invalid sender, the sender should be the contact",
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
