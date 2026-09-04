use std::{sync::Arc};

use super::{
    Card,
    CachedResolver,
    DHTResolver,
    Registry,
    ResolutionCache
};
use crate::{
    dht::Node,
    CryptoIdentity,
    Identity,
    Result,
    SignedBuilder
};

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
}
impl Registry for DHTRegistry {
    async fn register<'a>(
        &'a self,
        identity: &'a CryptoIdentity,
        card: &'a Card,
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
            .with_keypair(identity.signature_keypair())
            .with_sequence_number(version)
            .build()?;

        self.node.store_value(&value, version, true).await
    }

    type Resolver = CachedResolver<DHTResolver>;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }
}
