//! Boson compact credentials and their builder.

mod credential;
pub(crate) mod builder;

pub use credential::{Credential, Subject};
pub use builder::CredentialBuilder;
