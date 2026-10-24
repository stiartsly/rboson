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

use std::fmt;
use std::future::Future;

use crate::{w3c::DIDDocument, Card, ResolutionResult, Resolver};
use boson_api::{CryptoIdentity, Id, Result};

/// A card a registry publisher keeps: its version and the parsed card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCard {
    version: i32,
    card: Option<Card>,
}

impl PublishedCard {
    pub fn new(version: i32, card: Option<Card>) -> Self {
        Self { version, card }
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn card(&self) -> Option<&Card> {
        self.card.as_ref()
    }
}

impl fmt::Display for PublishedCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublishedCard({}, {:?})", self.version, self.card)
    }
}

/// Publishes and resolves Boson [`Card`]s.
pub trait Registry: Send + Sync {
    /// Publishes a card at a version newer than `last_version`, the version kept
    /// locally, and the newest version visible to the backend.
    fn publish<'a>(
        &'a self,
        identity: &'a CryptoIdentity,
        card: &'a Card,
        last_version: i32,
    ) -> impl Future<Output = Result<i32>> + 'a;

    /// Publishes a card without a caller-known previous version.
    fn publish_card<'a>(
        &'a self,
        identity: &'a CryptoIdentity,
        card: &'a Card,
    ) -> impl Future<Output = Result<i32>> + 'a {
        self.publish(identity, card, -1)
    }

    /// Returns what this registry's publisher keeps for `id`.
    fn get_published<'a>(
        &'a self,
        id: &'a Id,
    ) -> impl Future<Output = Result<Option<PublishedCard>>> + 'a;

    /// Stops publishing the card of `id`; returns whether one was kept.
    fn unpublish<'a>(&'a self, id: &'a Id) -> impl Future<Output = Result<bool>> + 'a;

    /// Resolves a card, optionally bypassing cached answers.
    fn resolve<'a>(
        &'a self,
        id: &'a Id,
        force: bool,
    ) -> impl Future<Output = Result<ResolutionResult<Card>>> + 'a;

    /// Resolves a card and converts a successful result to a DID document.
    fn resolve_document<'a>(
        &'a self,
        id: &'a Id,
        force: bool,
    ) -> impl Future<Output = Result<ResolutionResult<DIDDocument>>> + 'a {
        async move {
            let result = self.resolve(id, force).await?;
            Ok(ResolutionResult {
                status: result.status,
                result: result.result.as_ref().map(DIDDocument::from_card),
                metadata: result.metadata,
            })
        }
    }
}

/// Registry types that expose their underlying resolver.
pub trait RegistryResolver {
    type Resolver: Resolver;

    fn resolver(&self) -> &Self::Resolver;
}
