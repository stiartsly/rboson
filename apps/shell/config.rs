use std::env;

use boson::{
    director::{Options as DirectorOptions, UserRegistration},
    signature::{KeyPair, PrivateKey},
    Id, Result,
};

pub(crate) const BOSON_USER_KEY: &str = "BOSON_USER_KEY";
pub(crate) const BOSON_DIRECTOR_URL: &str = "BOSON_DIRECTOR_URL";

const DEFAULT_DIRECTOR_URL: &str = "https://47.101.142.224:9000";
const DEFAULT_DIRECTOR_NODE_ID: &str = "GhVW54uEd179PzRPpaiENKZuMezMNExTP6bXRK3rLDAQ";
const DEFAULT_USER_PRIVATE_KEY: &str = "0xbc12dc1054f83fcf0eba7720b706b369b58069c7e3b453be8d1f5493f69d72d13410da72883c6da7a6be00c2e5d70bbbf9a90c10bb099d01a656af2955e4af79";
const DEFAULT_USER_NAME: &str = "Alice";
const DEFAULT_USER_EMAIL: &str = "dummpy@example.com";
const DEFAULT_USER_BIO: &str = "Boson user";
const DEFAULT_USER_PASSPHRASE: &str = "test_secret_123";

#[derive(Clone)]
pub(crate) struct ShellConfig {
    director_url: String,
    director_node_id: Id,
    user_key: PrivateKey,
    insecure: bool,
}

impl ShellConfig {
    pub(crate) fn new(
        director_url: Option<&str>,
        userkey: Option<&str>,
        insecure: bool,
    ) -> Result<Self> {
        let director_url = director_url
            .map(str::to_owned)
            .or_else(|| env::var(BOSON_DIRECTOR_URL).ok())
            .unwrap_or_else(|| DEFAULT_DIRECTOR_URL.to_string());

        let user_key = userkey
            .map(str::to_owned)
            .or_else(|| env::var(BOSON_USER_KEY).ok())
            .unwrap_or_else(|| DEFAULT_USER_PRIVATE_KEY.to_string());

        Ok(Self {
            director_url,
            director_node_id: DEFAULT_DIRECTOR_NODE_ID.parse()?,
            user_key: PrivateKey::try_from(user_key.as_str())?,
            insecure,
        })
    }

    pub(crate) fn user_private_key(&self) -> &PrivateKey {
        &self.user_key
    }

    pub(crate) fn user_id(&self) -> Id {
        Id::from(KeyPair::from(&self.user_key).public_key())
    }

    pub(crate) fn director_options(&self) -> Result<DirectorOptions> {
        Ok(DirectorOptions::new(&self.director_url)?
            .with_node_id(self.director_node_id.clone())
            .with_user_id(self.user_id())
            .with_user_private_key(self.user_key.clone())
            .with_registration(
                UserRegistration::new()
                    .with_name(DEFAULT_USER_NAME)
                    .with_email(DEFAULT_USER_EMAIL)
                    .with_bio(DEFAULT_USER_BIO)
                    .with_passphrase(DEFAULT_USER_PASSPHRASE),
            )
            .with_insecure(self.insecure))
    }
}
