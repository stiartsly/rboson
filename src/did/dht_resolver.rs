use std::{
    sync::Arc,
    time::SystemTime
};

use crate::{
    dht::{LookupOption, Node},
    Id, Result,
};
use super::{
    Card,
    ResolutionMetadata,
    ResolutionOptions,
    ResolutionResult,
    Resolver
};

pub struct DHTResolver {
    node: Arc<Node>,
}

impl DHTResolver {
    pub fn new(node: Arc<Node>) -> Self {
        Self { node }
    }
    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }
}

impl Resolver for DHTResolver {
    async fn resolve<'a>(
        &'a self,
        id: &'a Id,
        options: Option<ResolutionOptions>,
    ) -> Result<ResolutionResult<Card>> {
        let lookup = if options.unwrap_or_default().use_cache {
            LookupOption::Arbitrary
        } else {
            LookupOption::Optimistic
        };
        let value = self.node.find_value(id, -1, Some(lookup)).await?;
        let Some(value) = value else {
            return Ok(ResolutionResult::not_found());
        };
        if value.public_key() != Some(id) || !value.is_valid() {
            return Ok(ResolutionResult::invalid());
        }
        let card = match Card::try_from(value.data()) {
            Ok(c) => c,
            Err(_) => return Ok(ResolutionResult::invalid()),
        };
        if card.id() != id || !card.is_genuine() {
            return Ok(ResolutionResult::invalid());
        }

        let metadata = ResolutionMetadata {
            created: card.signed_at(),
            updated: card.signed_at(),
            resolved: SystemTime::now(),
            deactivated: false,
            version: value.sequence_number(),
        };
        Ok(ResolutionResult::success(
            card.clone(),
            Some(metadata),
        ))
    }
}
