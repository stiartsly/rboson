pub mod core;
pub mod did;
pub mod dht;
//pub mod activeproxy;
//pub mod messaging;

pub use crate::core::{
    id::{
        self,
        Id,
        DID_PREFIX,
    },

    errors::{self, Error, Result},
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

    //node_config::{self, NodeConfig},
    //default_configuration as configuration,
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
    node::{self, Node},
    node_status::{self, NodeStatus}
};

/*
pub use crate::activeproxy::{
    ActiveProxyClient
};
*/

#[macro_export]
macro_rules! locked {
    ($mutex:expr) => {{
        $mutex.lock().unwrap()
    }};
}

#[macro_export]
macro_rules! elapsed_ms {
    ($time:expr) => {{
        $time.elapsed().unwrap_or(std::time::Duration::MAX).as_millis() as u128
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
use crate::errors::NetworkError;
use crate::errors::IOError;
fn local_addr(ipv4: bool) -> Result<IpAddr>{
    let if_addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => return Err(e.into()),
    };

    for iface in if_addrs {
        let ip = iface.ip();
        if !ip.is_loopback() &&
            ((ipv4 && ip.is_ipv4()) ||
            (!ipv4 && ip.is_ipv6())) {
            return Ok(ip)
        }
    }
    Err(NetworkError::new("No working network interfaces".into()))
}

fn create_dirs(input: &str) -> crate::core::Result<()> {
    let path = std::path::Path::new(input);
    if path.exists() {
        return Ok(())
    }

    std::fs::create_dir_all(path).map_err(|e|
        IOError::new(format!("Creating directory path {} error: {e}", input)).into()
    )
}

fn random_array<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    unsafe {
        libsodium_sys::randombytes_buf(
            bytes.as_mut_ptr() as *mut libc::c_void,
            N
        );
    };
    bytes
}

#[allow(unused)]
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

#[allow(unused)]
fn dump_hex(label: &str, data: &[u8]) {
    use hex::ToHex;
    let data_hex = data.encode_hex::<String>();
    println!("dumping(hex) {}: {}", label, data_hex);
}

/*
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
*/
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

/*
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
*/
