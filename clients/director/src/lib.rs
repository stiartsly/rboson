mod client;
mod device;
mod errors;
mod options;
mod pow;

mod avatar;
mod node_status;
mod plan;
mod profile;
mod profile_update;
mod subscription;
mod user_plan;
mod user_registration;

pub use {
    avatar::Avatar,
    client::Client,
    device::Device,
    errors::*,
    node_status::{NodeStatus, Service},
    options::DirectorOptions as Options,
    plan::{Cycle, Plan},
    profile::Profile,
    profile_update::ProfileUpdate,
    subscription::{Status, Subscription},
    user_plan::UserPlan,
    user_registration::UserRegistration,
};

#[cfg(test)]
mod unitests {
    mod test_client;
}

pub(crate) fn base64url(bytes: &[u8]) -> String {
    use base64::Engine;

    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn sign_nonce(
    key: &crate::signature::PrivateKey,
    nonce: &[u8],
) -> Result<String, crate::Error> {
    Ok(base64url(&key.sign_into(nonce)?))
}
