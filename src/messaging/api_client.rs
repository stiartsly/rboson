use std::cell::RefCell;
use reqwest::Client;
use url::Url;
use serde::{Serialize, Deserialize};

use crate::{
    unwrap,
    Identity,
    Id,
    Error,
    error::Result,
};

use crate::core::{
    crypto_identity::CryptoIdentity,
    cryptobox::Nonce,
};

use super::profile;

#[derive(Serialize, Debug)]
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
struct GetAccessTokenRspData {
    token   : Option<String>,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RegisterUserAndDeviceReqData {
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
struct RegisterDeviceWithUserReqData {
    userId: Id,
    passphrase: String,
    deviceId: Id,
    deviceName: String,
    appName: String,
    nonce: Vec<u8>,
    userSig: Vec<u8>,
    deviceSig: Vec<u8>
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ServiceIdsRspData {
    peerId: Option<String>,
    nodeId: Option<String>
}

#[allow(unused)]
pub(crate) struct APIClient {
    home_peerid : Id,
    base_url    : Url,
    client      : Client,

    user        : Option<CryptoIdentity>,
    device      : Option<CryptoIdentity>,
    access_token: Option<String>,

    access_token_refresh_handler: Option<Box<dyn Fn(&str)>>,

    nonce       : RefCell<Nonce>,
}

#[allow(unused)]
impl APIClient {
    pub(crate) fn new(peerid: &Id, base_url: &str) -> Result<Self> {
        let url = Url::parse(base_url).map_err(|e| {
            Error::Argument(format!("Invalid base url: {e}"))
        })?;

        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(Error::Argument("Invalid base url: scheme must be http or https".into()));
        }

        Ok(Self {
            home_peerid : peerid.clone(),
            base_url: url,
            client  : Client::builder().build().unwrap(),

            user    : None,
            device  : None,
            access_token: None,

            access_token_refresh_handler: None,

            nonce   : RefCell::new(Nonce::random())
        })
    }

    pub(crate) fn with_user_identity(&mut self, user: &CryptoIdentity) -> &Self {
        self.user = Some(user.clone());
        self
    }

    pub(crate) fn with_device_identity(&mut self, device: &CryptoIdentity) -> &Self {
        self.device = Some(device.clone());
        self
    }

    fn with_access_token(&mut self, access_token: &str) -> &Self {
        self.access_token = Some(access_token.to_string());
        self
    }

    pub(crate) fn with_access_token_refresh_handler(&mut self, handler: fn(&str)) -> &Self {
        self.access_token_refresh_handler = Some(Box::new(handler));
        self
    }

    /*pub(crate) fn user(&self) -> Option<&CryptoIdentity> {
        self.user.as_ref()
    }

    pub(crate) fn device(&self) -> Option<&CryptoIdentity> {
        self.device.as_ref()
    }*/

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
        )?.json::<ServiceIdsRspData>().await.map_err(|e| {
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

    async fn refresh_access_token(&mut self) -> Result<String> {
        assert!(self.user.is_some());
        assert!(self.device.is_some());

        let nonce = self.nonce();
        let user = unwrap!(self.user);
        let device = unwrap!(self.device);

        let data = RefreshAccessTokenReqData {
            userId  : user.id().clone(),
            deviceId: device.id().clone(),
            nonce   : nonce.borrow().as_bytes().to_vec(),
            userSig : user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig: device.sign_into(nonce.borrow().as_bytes()).unwrap()
        };

        let url = self.base_url.join("/api/v1/auth").unwrap();
        let result = self.client.post(url)
            .json(&data)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send()
            .await;

        let data = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<GetAccessTokenRspData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let Some(token) = data.token else {
            return Err(Error::State("Http error: missing access token in the response body".into()));
        };

        self.access_token = Some(token.clone());
        Ok(token)
    }

    pub(crate) async fn register_user_and_device(&mut self,
        passphrase: &str,
        user_name: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<String> {
        let nonce = self.nonce();
        let user = unwrap!(self.user);
        let device = unwrap!(self.device);

        let profile_digest = profile::digest(user.id(), &self.home_peerid, Some(user_name), false, None);
        let data = RegisterUserAndDeviceReqData {
            userId: user.id().clone(),
            userName: user_name.to_string(),
            passphrase: passphrase.to_string(),
            deviceId: device.id().clone(),
            deviceName: device_name.to_string(),
            appName:  app_name.to_string(),
            nonce: nonce.borrow().as_bytes().to_vec(),
            userSig: user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig: device.sign_into(nonce.borrow().as_bytes()).unwrap(),
            profileSig: user.sign_into(&profile_digest).unwrap(),
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
        )?.json::<GetAccessTokenRspData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let Some(token) = data.token else {
            return Err(Error::State("Http error: missing access token in the response body".into()));
        };

        self.access_token = Some(token.clone());
        Ok(token)
    }

    pub(crate) async fn register_device_with_user(&mut self,
        passphrase: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<String> {

        let nonce = self.nonce();
        let user = unwrap!(self.user);
        let device = unwrap!(self.device);
        let data = RegisterDeviceWithUserReqData {
            userId: user.id().clone(),
            passphrase: passphrase.to_string(),
            deviceId: device.id().clone(),
            deviceName: device_name.to_string(),
            appName:  app_name.to_string(),
            nonce: nonce.borrow().as_bytes().to_vec(),
            userSig: user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig: device.sign_into(nonce.borrow().as_bytes()).unwrap(),
        };

        let url = self.base_url.join("/api/v1/devices").unwrap();
        let result = self.client.post(url)
            .json(&data)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .send()
            .await;

        let rsp = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?;

        let rsp = rsp.json::<GetAccessTokenRspData>().await;
        let rspdata = rsp.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let Some(token) = rspdata.token else {
            return Err(Error::State("Http error: missing access token in the response body".into()));
        };

        self.access_token_refresh_handler.as_ref().map(|v| {
            v(&token);
        });

        Ok(token)
    }
}

#[allow(unused)]
mod base64_as_string {
    use serde::{Deserializer, Serializer};
    use serde::de::{Error, Deserialize};
    use base64::{engine::general_purpose, Engine as _};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(D::Error::custom)
    }
}
