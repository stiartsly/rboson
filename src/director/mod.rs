mod client;
mod device;
mod errors;
mod pow;

mod node_status;
mod user_registration;
mod avatar;
mod plan;
mod user_plan;
mod profile;
mod profile_update;
mod subscription;

pub use {
    client::{DirectorClient, DirectorClientBuilder},
    errors::*,
    device::Device,
    node_status::{NodeStatus, Service},
    user_registration::UserRegistration,
    avatar::Avatar,
    plan::{Plan, Cycle},
    user_plan::UserPlan,
    profile::Profile,
    profile_update::ProfileUpdate,
    subscription::{Subscription, Status},
};

#[cfg(test)]
mod unitests {
    mod test_client;
    //mod unitests;
}

pub(crate) fn base64url(bytes: &[u8]) -> String {
    use base64::Engine;

    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn sign_nonce(
    key: &crate::signature::KeyPair,
    nonce: &[u8],
) -> Result<String, crate::Error> {
    Ok(base64url(&key.private_key().sign_into(nonce)?))
}
