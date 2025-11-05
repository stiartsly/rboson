pub mod core;
pub mod did;
pub mod dht;
pub mod appdata_store;
pub mod activeproxy;
pub mod messaging;

pub use crate::core::{
    id::{
        self,
        Id,
        DID_PREFIX,
        ID_BYTES,
        ID_BITS
    },

    error::{self, Error},
    prefix::{self, Prefix},
    signature::{self, Signature},
    cryptobox::{self, CryptoBox},
    node_info::{self, NodeInfo},
    peer_info::{self, PeerInfo, PeerBuilder},
    value::{
        self,
        Value,
        ValueBuilder,
        SignedBuilder,
        EncryptedBuilder
    },
    network::{self, Network},
    identity::{self, Identity},
    crypto_identity::{self, CryptoIdentity},
    crypto_context::{self, CryptoContext},
    joint_result::{self, JointResult},

    config,
    default_configuration as configuration,
};

pub use crate::did::{
    didurl,
    verification_method,
    proof,
    w3c,
    credential,
    credential_builder,
    vouch,
    vouch_builder,
    card,
    card_builder,
};

pub use crate::dht::{
    node::{self, Node}
};

pub use crate::activeproxy::{
    ActiveProxyClient
};

#[macro_export]
macro_rules! elapsed_ms {
    ($time:expr) => {{
        $time.elapsed().unwrap().as_millis() as u128
    }};
}

#[macro_export]
macro_rules! as_ms {
    ($time:expr) => {{
        $time.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
    }};
}

#[macro_export]
macro_rules! as_secs {
    ($time:expr) => {{
        $time.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
    }};
}

#[macro_export]
macro_rules! unwrap {
    ($val:expr) => {{
        $val.as_ref().unwrap()
    }};
}

#[macro_export]
macro_rules! unwrap_mut {
    ($val:expr) => {{
        $val.as_mut().unwrap()
    }};
}

use std::net::IpAddr;
fn local_addr(ipv4: bool) -> crate::core::Result<IpAddr>{
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => return Err(Error::from(e))
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() &&
            ((ipv4 && ip.is_ipv4()) ||
            (!ipv4 && ip.is_ipv6())) {
            return Ok(ip)
        }
    }
    Err(Error::Network(format!("No working network interfaces")))
}

fn create_dirs(input: &str) -> crate::core::Result<()> {
    let path = std::path::Path::new(input);
    if path.exists() {
        return Ok(())
    }

    std::fs::create_dir_all(path).map_err(|e|
         Error::Io(format!("Creating directory path {} error: {e}", input))
    )
}

#[cfg(test)]
fn remove_working_path(input: &str) {
    if std::fs::metadata(&input).is_ok() {
        match std::fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}

fn randomize_bytes<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        libsodium_sys::randombytes_buf(
            array.as_mut_ptr() as *mut libc::c_void,
            N
        );
    }
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    unsafe {
        libsodium_sys::randombytes_buf(
            bytes.as_mut_ptr() as *mut libc::c_void,
            len
        );
    };
    bytes
}

pub(crate) fn is_none_or_empty<T: IsEmpty>(v: &Option<T>) -> bool {
    v.as_ref().map(|s| s.is_empty()).unwrap_or(true)
}

pub(crate) fn is_empty<T: IsEmpty>(v: &T) -> bool {
    v.is_empty()
}

trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl IsEmpty for String {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for u64 {
    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl IsEmpty for bool {
    fn is_empty(&self) -> bool {
        !*self
    }
}

// serde Id as base58 string
mod serde_id_as_base58 {
    use crate::Id;
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use bs58;

    pub fn serialize<S>(id: &Id, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = bs58::encode(id.as_bytes()).into_string();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Id, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Id::try_from(s.as_str()).map_err(D::Error::custom)?)
    }
}

mod serde_id_as_bytes {
    use crate::Id;
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};

    pub fn serialize<S>(id: &Id, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(id.as_bytes())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Id, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <&[u8]>::deserialize(deserializer)?;

        if bytes.len() != crate::ID_BYTES {
            return Err(D::Error::invalid_length(bytes.len(), &format!("{}", crate::ID_BYTES).as_str()));
        }

        let mut arr = [0u8; crate::ID_BYTES];
        arr.copy_from_slice(bytes);
        Ok(Id::from_bytes(arr))
    }
}

mod serde_option_id_as_base58 {
    use crate::Id;
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use bs58;

    pub fn serialize<S>(id: &Option<Id>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match id.as_ref() {
            None => serializer.serialize_none(),
            Some(id) => {
                let s = bs58::encode(id.as_bytes()).into_string();
                serializer.serialize_str(&s)
            }
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Id>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Some(Id::try_from(s.as_str()).map_err(D::Error::custom)?))
    }
}

// bytes serded as base64 URL safe without padding
mod serde_bytes_base64 {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use base64::{engine::general_purpose, Engine as _};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
    {
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(D::Error::custom)
    }
}

mod serde_option_bytes_as_cbor {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};

    pub fn serialize<S>(bytes: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match bytes.as_ref() {
            Some(bytes) => serializer.serialize_bytes(bytes.as_slice()),
            None => serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <&[u8]>::deserialize(deserializer).map_err(D::Error::custom)?;
        Ok(Some(Vec::<u8>::from(s)))
    }
}
