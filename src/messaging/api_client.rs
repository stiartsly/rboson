use std::cell::RefCell;
use reqwest::Client;
use url::Url;
use serde::{Serialize, Deserialize};

use crate::{
    Id,
    Error,
    error::Result,
    Identity,
};

use crate::core::{
    crypto_identity::CryptoIdentity,
    cryptobox::Nonce,
};

use super::profile;

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RefreshAccessTokenReqData {
    userId  : Id,
    deviceId: Id,
    #[serde(with = "base64_as_string")]
    nonce   : Vec<u8>,
    #[serde(with = "base64_as_string")]
    userSig : Vec<u8>,
    #[serde(with = "base64_as_string")]
    deviceSig: Vec<u8>
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct AccessTokenData {
    token   : Option<String>,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RegisterUserData {
    userId  : Id,
    userName: String,
    passphrase  : String,
    deviceId: Id,
    deviceName  : String,
    appName : String,
    #[serde(with = "base64_as_string")]
    nonce   : Vec<u8>,
    #[serde(with = "base64_as_string")]
    userSig : Vec<u8>,
    #[serde(with = "base64_as_string")]
    deviceSig   : Vec<u8>,
    #[serde(with = "base64_as_string")]
    profileSig  : Vec<u8>
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RegisterDeviceData {
    userId: Id,
    passphrase: String,
    deviceId: Id,
    deviceName: String,
    appName: String,
    #[serde(with = "base64_as_string")]
    nonce: Vec<u8>,
    #[serde(with = "base64_as_string")]
    userSig: Vec<u8>,
    #[serde(with = "base64_as_string")]
    deviceSig: Vec<u8>
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ServiceIdsData {
    peerId: Option<String>,
    nodeId: Option<String>
}

pub(crate) struct Builder<'a> {
    home_peerid : Option<&'a Id>,
    base_url    : Option<&'a str>,
    user        : Option<&'a CryptoIdentity>,
    device      : Option<&'a CryptoIdentity>,
}

impl<'a> Builder<'a> {
    pub(crate) fn new() -> Self {
        Self {
            home_peerid : None,
            base_url    : None,
            user    : None,
            device  : None,
        }
    }

    pub(crate) fn with_home_peerid(mut self, peerid: &'a Id) -> Self {
        self.home_peerid = Some(peerid);
        self
    }

    pub(crate) fn with_base_url(mut self, base_url: &'a str) -> Self {
        self.base_url = Some(base_url);
        self
    }

    pub(crate) fn with_user_identity(mut self, user: &'a CryptoIdentity) -> Self {
        self.user = Some(user);
        self
    }

    pub(crate) fn with_device_identity(mut self, device: &'a CryptoIdentity) -> Self {
        self.device = Some(device);
        self
    }

    pub(crate) fn build(self) -> Result<APIClient> {
        assert!(self.home_peerid.is_some());
        assert!(self.base_url.is_some());
        assert!(self.user.is_some());
        assert!(self.device.is_some());

        APIClient::new(self)
    }
}

pub(crate) struct APIClient {
    client      : Client,
    peerid      : Id,
    base_url    : Url,
    user        : CryptoIdentity,
    device      : CryptoIdentity,

    access_token: Option<String>,
    access_token_refresh_handler: Option<Box<dyn Fn(&str)>>,

    nonce       : RefCell<Nonce>,
}

#[allow(unused)]
impl APIClient {
    pub(crate) fn new(b: Builder) -> Result<Self> {
        let url = Url::parse(b.base_url.as_ref().unwrap()).map_err(|e| {
            Error::Argument(format!("Invalid base url: {e}"))
        })?;

        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(Error::Argument("Invalid base url: scheme must be http or https".into()));
        }


        Ok(Self {
            client  : Client::builder().build().unwrap(),
            peerid  : b.home_peerid.unwrap().clone(),
            base_url: url,
            user    : b.user.unwrap().clone(),
            device  : b.device.unwrap().clone(),

            access_token: None,
            access_token_refresh_handler: None,

            nonce   : RefCell::new(Nonce::random())
        })
    }

    fn with_access_token(&mut self, access_token: &str) -> &Self {
        self.access_token = Some(access_token.to_string());
        self
    }

    pub(crate) fn with_access_token_refresh_handler(&mut self, handler: fn(&str)) -> &Self {
        self.access_token_refresh_handler = Some(Box::new(handler));
        self
    }

    pub(crate) fn user(&self) -> &CryptoIdentity {
        &self.user
    }

    pub(crate) fn device(&self) -> &CryptoIdentity {
        &self.device
    }

    pub(crate) fn access_token(&self) -> Option<&str> {
        self.access_token.as_ref().map(|v|v.as_str())
    }

    pub(crate) async fn service_ids(&self) -> Result<(Id, Id)> {
        let url = self.base_url.join("/api/v1/service/id").unwrap();
        let result = self.client.get(url)
            .header("Accept", "application/json")
            .send()
            .await;

        let data = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<ServiceIdsData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let Some(peerid_str) = data.peerId else {
            return Err(Error::State("Http error: missing peer id".into()));
        };
        let Some(nodeid_str) = data.nodeId else {
            return Err(Error::State("Http error: missing nodeid id".into()));
        };

        let Ok(peerid) = Id::try_from(peerid_str.as_str()) else {
            return Err(Error::State("Http error: invalid peer id".into()));
        };
        let Ok(nodeid) = Id::try_from(nodeid_str.as_str()) else {
            return Err(Error::State("Http error: invalid node id".into()));
        };

        Ok((peerid, nodeid))
    }

    fn nonce(&self) -> &RefCell<Nonce> {
        self.nonce.borrow_mut().increment();
        &self.nonce
    }

    async fn refresh_access_token(&mut self) -> Result<()> {
        let nonce = self.nonce();

        let data = RefreshAccessTokenReqData {
            userId      : self.user.id().clone(),
            deviceId    : self.device.id().clone(),
            nonce       : nonce.borrow().as_bytes().to_vec(),
            userSig     : self.user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig   : self.device.sign_into(nonce.borrow().as_bytes()).unwrap()
        };

        let url = self.base_url.join("/api/v1/auth").unwrap();
        let result = self.client.post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;

        let data = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<AccessTokenData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        self.access_token = Some(data.token.unwrap());
        Ok(())
    }

    pub(crate) async fn register_user_and_device(&mut self,
        passphrase: &str,
        user_name: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<()> {
        let nonce = self.nonce();
        let profile_digest = profile::digest(self.user.id(), &self.peerid, Some(user_name), false, None);
        let data = RegisterUserData {
            userId      : self.user.id().clone(),
            userName    : user_name.to_string(),
            passphrase  : passphrase.to_string(),
            deviceId    : self.device.id().clone(),
            deviceName  : device_name.to_string(),
            appName     : app_name.to_string(),
            nonce       : nonce.borrow().as_bytes().to_vec(),
            userSig     : self.user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig   : self.device.sign_into(nonce.borrow().as_bytes()).unwrap(),
            profileSig  : self.user.sign_into(&profile_digest).unwrap(),
        };

        let url = self.base_url.join("/api/v1/users").unwrap();
        let result = self.client.post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;

        let data = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<AccessTokenData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        self.access_token = Some(data.token.unwrap());
        Ok(())
    }

    pub(crate) async fn register_device_with_user(&mut self,
        passphrase: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<String> {
        let nonce = self.nonce();
        let data = RegisterDeviceData {
            userId      : self.user.id().clone(),
            passphrase  : passphrase.to_string(),
            deviceId    : self.device.id().clone(),
            deviceName  : device_name.to_string(),
            appName     : app_name.to_string(),
            nonce       : nonce.borrow().as_bytes().to_vec(),
            userSig     : self.user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig   : self.device.sign_into(nonce.borrow().as_bytes()).unwrap(),
        };

        let url = self.base_url.join("/api/v1/devices").unwrap();
        let result = self.client.post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;

        let data = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<AccessTokenData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let token = data.token.unwrap();
        self.access_token_refresh_handler.as_ref().map(|v| {
            v(&token);
        });
        Ok(token)
    }
}

mod base64_as_string {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use base64::{engine::general_purpose, Engine as _};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
    {
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    #[allow(unused)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(D::Error::custom)
    }
}
