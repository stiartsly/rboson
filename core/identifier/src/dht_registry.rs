use std::sync::Arc;

use super::{
    CachedResolver, Card, DHTResolver, PublishedCard, Registry, RegistryResolver, ResolutionCache,
    ResolutionOptions, ResolutionResult, Resolver,
};
use crate::{dht::Node, CryptoIdentity, Id, Identity, Result, SignedBuilder};

pub struct DHTRegistry {
    node: Arc<Node>,
    resolver: CachedResolver<DHTResolver>,
}

impl DHTRegistry {
    pub fn new(node: Arc<Node>, cache: Option<Arc<dyn ResolutionCache>>) -> Self {
        let base = DHTResolver::new(node.clone());
        Self {
            node,
            resolver: CachedResolver::new(base, cache),
        }
    }

    pub fn resolver(&self) -> &CachedResolver<DHTResolver> {
        &self.resolver
    }

    /// Stores a card at an explicitly selected version.
    pub async fn register(
        &self,
        identity: &CryptoIdentity,
        card: &Card,
        version: i32,
    ) -> Result<()> {
        if version < 0 {
            return Err("version must be non-negative".into());
        }
        if identity.id() != card.id() {
            return Err("identity id does not match card id".into());
        }
        if !card.is_genuine() {
            return Err("card is not genuine".into());
        }

        let data: Vec<u8> = card.into();
        let value = SignedBuilder::new(&data)
            .with_keypair(identity.signature_key())
            .with_sequence_number(version)
            .build()?;

        self.node.store_value(&value, version, true).await
    }
}

impl Registry for DHTRegistry {
    async fn publish(
        &self,
        identity: &CryptoIdentity,
        card: &Card,
        last_version: i32,
    ) -> Result<i32> {
        if identity.id() != card.id() {
            return Err("identity id does not match card id".into());
        }
        if !card.is_genuine() {
            return Err("card is not genuine".into());
        }

        let kept = self.node.value(*card.id())?;
        let found = self
            .node
            .find_value(card.id(), -1, None)
            .await
            .ok()
            .flatten();
        let highest = last_version
            .max(kept.map(|value| value.sequence_number()).unwrap_or(-1))
            .max(found.map(|value| value.sequence_number()).unwrap_or(-1));
        let version = highest
            .checked_add(1)
            .ok_or_else(|| crate::errors::ArgumentError::new("card version is exhausted"))?;

        self.register(identity, card, version).await?;
        self.resolver.invalidate(card.id());
        Ok(version)
    }

    async fn get_published(&self, id: &Id) -> Result<Option<PublishedCard>> {
        Ok(self.node.value(*id)?.map(|value| {
            let card = Card::try_from(value.data()).ok();
            PublishedCard::new(value.sequence_number(), card)
        }))
    }

    async fn unpublish(&self, id: &Id) -> Result<bool> {
        let existed = self.node.value(*id)?.is_some();
        self.node.remove_value(*id)?;
        self.resolver.invalidate(id);
        Ok(existed)
    }

    async fn resolve(&self, id: &Id, force: bool) -> Result<ResolutionResult<Card>> {
        let options = ResolutionOptions {
            use_cache: !force,
            ..ResolutionOptions::default()
        };
        self.resolver.resolve(id, Some(options)).await
    }
}

impl RegistryResolver for DHTRegistry {
    type Resolver = CachedResolver<DHTResolver>;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }
}
