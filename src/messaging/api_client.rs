use reqwest::Client;
use url::Url;
use serde::{Serialize, Deserialize};

use crate::{
    Id,
    Error,
    error::Result,
    Identity,
    PeerInfo,
    cryptobox::Nonce,
    core::crypto_identity::CryptoIdentity,
};

use crate::messaging::{
    UserProfile,
    ServiceIds,
};

use crate::messaging::{
    profile::{self, Profile},
    service_ids::JsonServiceIds,
    internal::contact_update::ContactsUpdate,
};

static HTTP_HEADER_ACCEPT: &str = "Accept";
static HTTP_HEADER_CONTENT_TYPE: &str = "Content-Type";
static HTTP_BODY_FORMAT_JSON: &str = "application/json";

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
        };
        if self.user.is_none() {
            return Err(Error::Argument("User identity is required".into()));
        };
        if self.device.is_none() {
            return Err(Error::Argument("Device identity is required".into()));
        };

        let Some(base_url) = self.base_url else {
            return Err(Error::Argument("Base url is required".into()));
        };
        let base_url = Url::parse(base_url).map_err(|e| {
            Error::Argument(format!("Invalid base url: {e}"))
        })?;
        if base_url.scheme() != "http" && base_url.scheme() != "https" {
            return Err(Error::Argument("Invalid base url: scheme must be http or https".into()));
        }

        APIClient::new(self, base_url)
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

    nonce       : Nonce,
}

#[allow(unused)]
impl APIClient {
    pub(crate) fn new(b: Builder, base_url: Url) -> Result<Self> {
        let result = Client::builder()
            .user_agent("rboson")
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let client = result.map_err(|e|
            Error::Argument(format!("Failed to create http client: {e}"))
        )?;

        Ok(Self {
            client,
            base_url,
            peerid  : b.home_peerid.unwrap().clone(),
            user    : b.user.unwrap().clone(),
            device  : b.device.unwrap().clone(),

            access_token: None,
            access_token_refresh_handler: None,

            nonce   : Nonce::random(),
        })
    }

     pub(crate) fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    pub(crate) fn set_access_token(&mut self, token: String) {
        self.access_token = Some(token);
    }

    pub(crate) fn set_access_token_refresh_handler(&mut self, handler: fn(&str)) {
        self.access_token_refresh_handler = Some(Box::new(handler));
    }

    fn increment_nonce(&mut self) -> Nonce {
        self.nonce.increment();
        self.nonce.clone()
    }

    pub(crate) async fn service_ids(base_url: &Url) -> Result<ServiceIds> {
        let url = base_url.join("/api/v1/service/id").unwrap();
        let result = Client::builder()
            .user_agent("rboson")
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let client = result.map_err(|e|
            Error::Argument(format!("Failed to create http client: {e}"))
        )?;

        let rsp = client.get(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .send()
            .await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<JsonServiceIds>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;
        data.ids()
    }

    async fn refresh_access_token(&mut self) -> Result<()> {
        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct RequestData<'a> {
            userId      : &'a Id,
            deviceId    : &'a Id,
            #[serde(with = "super::serde_bytes_with_base64")]
            nonce       : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            userSig     : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            deviceSig   : &'a [u8],
        }

        #[derive(Deserialize)]
        struct ResponseData {
            token       : String,
        }

        let nonce = self.increment_nonce();
        let usr_sig = self.user.sign_into(nonce.as_bytes()).unwrap();
        let dev_sig = self.device.sign_into(nonce.as_bytes()).unwrap();
        let data = RequestData {
            userId      : self.user.id(),
            deviceId    : self.device.id(),
            nonce       : nonce.as_bytes(),
            userSig     : usr_sig.as_slice(),
            deviceSig   : dev_sig.as_slice(),
        };

        let url = self.base_url.join("/api/v1/auth").unwrap();
        let rsp = self.client.post(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        self.access_token = Some(data.token);
        Ok(())
    }

    pub(crate) async fn register_user_with_device(&mut self,
        passphrase: &str,
        user_name: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<()> {

        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct RequestData<'a> {
            userId      : &'a Id,
            userName    : &'a str,
            passphrase  : &'a str,
            deviceId    : &'a Id,
            deviceName  : &'a str,
            appName     : &'a str,
            #[serde(with = "super::serde_bytes_with_base64")]
            nonce       : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            userSig     : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            deviceSig   : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            profileSig  : &'a [u8],
        }

        #[derive(Deserialize)]
        struct ResponseData {
            token       : String,
        }

        let nonce = self.increment_nonce();
        let usr_sig = self.user.sign_into(nonce.as_bytes()).unwrap();
        let dev_sig = self.device.sign_into(nonce.as_bytes()).unwrap();
        let profile_sig = self.user.sign_into(&profile::digest(
            self.user.id(),
            &self.peerid,
            Some(user_name),
            false,
            None
        )).unwrap();

        let data = RequestData {
            userId      : self.user.id(),
            userName    : user_name,
            passphrase  : passphrase,
            deviceId    : self.device.id(),
            deviceName  : device_name,
            appName     : app_name,
            nonce       : nonce.as_bytes(),
            userSig     : usr_sig.as_slice(),
            deviceSig   : dev_sig.as_slice(),
            profileSig  : profile_sig.as_slice(),
        };

        let url = self.base_url.join("/api/v1/users").unwrap();
        let rsp = self.client.post(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        self.access_token = Some(data.token);
        Ok(())
    }

    pub(crate) async fn register_device_with_user(&mut self,
        passphrase: &str,
        device_name: &str,
        app_name: &str
    ) -> Result<UserProfile> {

        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct RequestData<'a> {
            userId      : &'a Id,
            passphrase  : &'a str,
            deviceId    : &'a Id,
            deviceName  : &'a str,
            appName     : &'a str,
            #[serde(with = "super::serde_bytes_with_base64")]
            nonce       : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            userSig     : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            deviceSig   : &'a [u8],
        }

        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct ResponseData {
            token       : String,
            userName    : String,
            avatar      : bool,
        }

        let nonce = self.increment_nonce();
        let usr_sig = self.user.sign_into(nonce.as_bytes()).unwrap();
        let dev_sig = self.device.sign_into(nonce.as_bytes()).unwrap();
        let data = RequestData {
            userId      : self.user.id(),
            passphrase  : passphrase,
            deviceId    : self.device.id(),
            deviceName  : device_name,
            appName     : app_name,
            nonce       : nonce.as_bytes(),
            userSig     : usr_sig.as_slice(),
            deviceSig   : dev_sig.as_slice(),
        };

        let url = self.base_url.join("/api/v1/devices").unwrap();
        let rsp = self.client.post(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let token = data.token;
        self.access_token_refresh_handler.as_ref().map(|v| {
            v(&token);
        });

        Ok(UserProfile::new(
            self.user.clone(),
            data.userName,
            data.avatar
        ))
    }

    pub(crate) async fn register_device_request(&mut self,
        device_name: &str,
        app_name: &str
    ) -> Result<String> {

        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct RequestData<'a> {
            deviceId    : &'a Id,
            deviceName  : &'a str,
            appName     : &'a str,
            #[serde(with = "super::serde_bytes_with_base64")]
            nonce       : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            sig         : &'a [u8],
        }

        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct ResponseData {
            registrationId: String,
        }

        let nonce = self.increment_nonce();
        let sig = self.device.sign_into(nonce.as_bytes()).unwrap();
        let data = RequestData {
            deviceId    : self.device.id(),
            deviceName  : device_name,
            appName     : app_name,
            nonce       : nonce.as_bytes(),
            sig         : sig.as_slice(),
        };

        let url = self.base_url.join("/api/v1/devices/registrations").unwrap();
        let rsp = self.client.post(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;
        Ok(data.registrationId)
    }

    pub(crate) async fn finish_register_device_request(&mut self,
        registration_id: &str,
        _timeout: u64
    ) -> Result<String> {
        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct RequestData<'a> {
            deviceId    : &'a Id,
            #[serde(with = "super::serde_bytes_with_base64")]
            nonce       : &'a [u8],
            #[serde(with = "super::serde_bytes_with_base64")]
            sig         : &'a [u8],
        }

        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct ResponseData {
            registrationId: String,
        }

        let nonce = self.increment_nonce();
        let sig = self.device.sign_into(nonce.as_bytes()).unwrap();
        let data = RequestData {
            deviceId    : self.device.id(),
            nonce       : nonce.as_bytes(),
            sig         : sig.as_slice(),
        };

        let path = format!("/api/v1/devices/registrations/{}", registration_id);
        let url = self.base_url.join(path.as_str()).unwrap();
        let rsp = self.client.post(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
           // .bearer_auth(self.access_token().unwrap())
            .json(&data)
            .send()
            .await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;
        Ok(data.registrationId)
    }

    pub(crate) async fn service_info(&self) -> Result<PeerInfo> {
        unimplemented!()
    }

    pub(crate) async fn update_profile(&mut self, name: &str, avatar: bool) -> Result<()> {
        #[derive(Serialize)]
        #[allow(non_snake_case)]
        struct RequestData<'a> {
            userName    : &'a str,
            avatar      : bool,
            #[serde(with = "super::serde_bytes_with_base64")]
            profileSig  : &'a [u8],
        }

        let digest = profile::digest(
            self.user.id(),
            &self.peerid,
            Some(name),
            avatar,
            None
        );
        let sig = self.user.sign_into(&digest).unwrap();

        let data = RequestData {
            userName    : name,
            avatar      : false, // TODO: handle avatar upload
            profileSig  : sig.as_slice(),
        };

        let url = self.base_url.join("/api/v1/profile").unwrap();
        let rsp = self.client.put(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .bearer_auth(self.access_token().unwrap())
            .json(&data)
            .send()
            .await;

        rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?;
        Ok(())
    }

    pub(crate) async fn get_profile(&mut self, id: &Id) -> Result<profile::Profile> {
        let path = format!("/api/v1/profile/{}", id);
        let url = self.base_url.join(path.as_str()).unwrap();
        let rsp = self.client.get(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .send().await;

        let data = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?.json::<Profile>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        Ok(data)
    }

    pub(crate) async fn upload_avatar(&mut self,
        _content_type: &str,
        _avatar: &[u8]
    ) -> Result<String> {
        unimplemented!()
    }

    pub(crate) async fn upload_avatar_with_filename(&mut self,
        _content_type: &str,
        _file_name: String,
    ) -> Result<String> {
        unimplemented!()
    }

    pub(crate) async fn fetch_contacts_update(&mut self,
        version_id: Option<&str>
    ) -> Result<ContactsUpdate> {
        unimplemented!()
    }
}
