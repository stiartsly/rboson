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

// Migrated code that is not documented yet. Remove this allow as the crate is brought in line
// with the workspace lints.
#![allow(missing_docs)]

mod client;
mod device;
mod errors;
mod options;

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
    key: &boson_api::signature::PrivateKey,
    nonce: &[u8],
) -> Result<String, boson_api::Error> {
    Ok(base64url(&key.sign_into(nonce)?))
}

/// Signs the message that authorizes adding a device: the device id, then the nonce.
pub(crate) fn sign_device_authorization(
    key: &boson_api::signature::PrivateKey,
    device_id: &boson_api::Id,
    nonce: &[u8],
) -> Result<String, boson_api::Error> {
    let mut message = Vec::with_capacity(boson_api::Id::BYTES + nonce.len());
    message.extend_from_slice(device_id.as_bytes());
    message.extend_from_slice(nonce);
    Ok(base64url(&key.sign_into(&message)?))
}
