use std::future::Future;

use super::{Card, Resolver};
use crate::{CryptoIdentity, Result};
pub trait Registry: Send + Sync {
    fn register<'a>(
        &'a self,
        identity: &'a CryptoIdentity,
        card: &'a Card,
        version: i32,
    ) -> impl Future<Output = Result<()>> + 'a;

    type Resolver: Resolver;

    fn resolver(&self) -> &Self::Resolver;
}
