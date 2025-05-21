use std::cell::RefCell;
use reqwest::Client;
use url::Url;
use serde::{Serialize, Deserialize};

use crate::{
    unwrap,
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
    userId      : Id,
    deviceId    : Id,
    #[serde(with = "base64_as_string")]
    nonce       : Vec<u8>,
    #[serde(with = "base64_as_string")]
    userSig     : Vec<u8>,
    #[serde(with = "base64_as_string")]
    deviceSig   : Vec<u8>
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ServiceIdsData {
    peerId: String,
    nodeId: String
}

impl ServiceIdsData {
    fn ids(&self) -> Result<(Id, Id)> {
        let Ok(peerid) = Id::try_from(self.peerId.as_str()) else {
            return Err(Error::State("Http error: invalid peer id".into()));
        };
        let Ok(nodeid) = Id::try_from(self.nodeId.as_str()) else {
            return Err(Error::State("Http error: invalid node id".into()));
        };
        Ok((peerid, nodeid))
    }
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct AccessTokenData {
    token       : String,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct UserData {
    userId      : Id,
    userName    : String,
    passphrase  : String,
    deviceId    : Id,
    deviceName  : String,
    appName     : String,
    #[serde(with = "base64_as_string")]
    nonce       : Vec<u8>,
    #[serde(with = "base64_as_string")]
    userSig     : Vec<u8>,
    #[serde(with = "base64_as_string")]
    deviceSig   : Vec<u8>,
    #[serde(with = "base64_as_string")]
    profileSig  : Vec<u8>
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct DeviceData {
    userId      : Id,
    passphrase  : String,
    deviceId    : Id,
    deviceName  : String,
    appName     : String,
    #[serde(with = "base64_as_string")]
    nonce       : Vec<u8>,
    #[serde(with = "base64_as_string")]
    userSig     : Vec<u8>,
    #[serde(with = "base64_as_string")]
    deviceSig   : Vec<u8>
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RegisterDeviceRequestData {
    deviceId    : Id,
    deviceName  : String,
    appName     : String,
    #[serde(with = "base64_as_string")]
    nonce       : Vec<u8>,
    #[serde(with = "base64_as_string")]
    sig         : Vec<u8>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct DeviceRegisterationData {
    registrationId: String,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct FinishRegisterDeviceRequestData {
    deviceId    : Id,
   #[serde(with = "base64_as_string")]
    nonce       : Vec<u8>,
    #[serde(with = "base64_as_string")]
    sig         : Vec<u8>,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct ProfileData {
    userName    : String,
    avatar      : bool,
    #[serde(with = "base64_as_string")]
    profileSig  : Vec<u8>,
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
        if self.home_peerid.is_none() {
            return Err(Error::Argument("Home peer id is required".into()));
        }
        if self.base_url.is_none() {
            return Err(Error::Argument("Base url is required".into()));
        }
        if self.user.is_none() {
            return Err(Error::Argument("User identity is required".into()));
        }
        if self.device.is_none() {
            return Err(Error::Argument("Device identity is required".into()));
        }

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
        let base_url = Url::parse(unwrap!(b.base_url)).map_err(|e| {
            Error::Argument("Invalid base url: {e}".into())
        })?;

        if base_url.scheme() != "http" && base_url.scheme() != "https" {
            return Err(Error::Argument("Invalid base url: scheme must be http or https".into()));
        }

        let client = Client::builder()
            .user_agent("boson-rs")
            .timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(30))
            .danger_accept_invalid_certs(false)
            .danger_accept_invalid_hostnames(false)
            .build()
            .map_err(|e| {
                Error::Argument(format!("Failed to create http client: {e}"))
            })?;

        Ok(Self {
            client,
            base_url,
            peerid  : b.home_peerid.unwrap().clone(),
            user    : b.user.unwrap().clone(),
            device  : b.device.unwrap().clone(),

            access_token: None,
            access_token_refresh_handler: None,

            nonce   : RefCell::new(Nonce::random())
        })
    }

    fn set_access_token(&mut self, access_token: &str) {
        self.access_token = Some(access_token.to_string());
    }

    pub(crate) fn set_access_token_refresh_handler(&mut self, handler: fn(&str)) {
        self.access_token_refresh_handler = Some(Box::new(handler));
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

        let data = result.map_err(|e|
            Error::State("Http error: sending http request error {e}".into())
        )?.error_for_status().map_err(|e|
            Error::State("Http error: invalid http response {e}".into())
        )?.json::<ServiceIdsData>().await.map_err(|e|
            Error::State("Http error: deserialize json error {e}".into())
        )?;

        data.ids()
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

        self.access_token = Some(data.token);
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
        let data = UserData {
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

        self.access_token = Some(data.token);
        Ok(())
    }

    pub(crate) async fn register_device_with_user(&mut self,
        passphrase: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<String> {
        let nonce = self.nonce();
        let data = DeviceData {
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

        let token = data.token;
        self.access_token_refresh_handler.as_ref().map(|v| {
            v(&token);
        });
        Ok(token)
    }

    pub(crate) async fn register_device_request(&mut self,
        device_name: &str,
        app_name: &str
    ) -> Result<String> {
        let nonce = self.nonce();
        let data = RegisterDeviceRequestData {
            deviceId    : self.device.id().clone(),
            deviceName  : device_name.to_string(),
            appName     : app_name.to_string(),
            nonce       : nonce.borrow().as_bytes().to_vec(),
            sig         : self.device.sign_into(nonce.borrow().as_bytes()).unwrap(),
        };

        let url = self.base_url.join("/api/v1/devices/registrations").unwrap();
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
        )?.json::<DeviceRegisterationData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        Ok(data.registrationId)
    }

    pub(crate) async fn finish_register_device_request(&mut self,
        registration_id: &str,
        _timeout: u64
    ) -> Result<String> {
        let nonce = self.nonce();
        let data = FinishRegisterDeviceRequestData {
            deviceId    : self.device.id().clone(),
            nonce       : nonce.borrow().as_bytes().to_vec(),
            sig         : self.device.sign_into(nonce.borrow().as_bytes()).unwrap(),
        };

        let url_path = format!("/api/v1/devices/registrations/{registration_id}/0");
        let url = self.base_url.join(&url_path).unwrap();
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
        )?.json::<DeviceRegisterationData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        Ok(data.registrationId)
    }

    pub(crate) async fn update_profile(&mut self,
        name: String,
        avatar: bool
    ) -> Result<()> {
        let nonce = self.nonce();
        let profile_digest = profile::digest(self.user.id(), &self.peerid, Some(&name), avatar, None);
        let data = ProfileData {
            userName    : name,
            avatar      : avatar,
            profileSig  : profile_digest,
        };

        let url_path = format!("/api/v1/profile");
        let url = self.base_url.join(&url_path).unwrap();
        let result = self.client.post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;

        result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<DeviceRegisterationData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        Ok(())
    }

    pub(crate) async fn upload_avatar(&mut self,
        content_type: &str,
        avatar: &[u8]
    ) -> Result<String> {
        unimplemented!()
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
