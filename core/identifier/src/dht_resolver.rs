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

use std::{sync::Arc, time::SystemTime};

use super::{Card, ResolutionMetadata, ResolutionOptions, ResolutionResult, Resolver};
use boson_dht::{
    {LookupOption, Node},
};
use boson_api::{
    Id,
    Result,
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
        Ok(ResolutionResult::success(card.clone(), Some(metadata)))
    }
}
