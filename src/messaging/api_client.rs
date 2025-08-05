use std::time::Duration;
use reqwest::{StatusCode, Client};
use serde::{Serialize, Deserialize};
use serde_json::Map;
use url::Url;
use log::warn;

use crate::{
    unwrap,
    Id,
    Error,
    cryptobox::Nonce,
    core::Result,
    core::CryptoIdentity,
};

use crate::messaging::{
    UserProfile,
    ServiceIds,
    profile::{self, Profile},
    service_ids::JsonServiceIds,
    internal::ContactsUpdate,
};

static HTTP_HEADER_ACCEPT: &str = "Accept";
static HTTP_HEADER_CONTENT_TYPE: &str = "Content-Type";
static HTTP_BODY_FORMAT_JSON: &str = "application/json";

pub(crate) struct Builder<'a> {
    peerid      : Option<&'a Id>,   // home peerid.
    base_url    : Option<&'a Url>,
    user        : Option<&'a CryptoIdentity>,
    device      : Option<&'a CryptoIdentity>,

    access_token: Option<&'a str>,
    access_token_refresh_handler: Option<Box<dyn Fn(&str)>>,
}

impl<'a> Builder<'a> {
    pub(crate) fn new() -> Self {
        Self {
            peerid      : None,
            base_url    : None,
            user        : None,
            device      : None,

            access_token: None,
            access_token_refresh_handler: None,
        }
    }

    pub(crate) fn with_home_peerid(&mut self, peerid: &'a Id) -> &mut Self {
        self.peerid = Some(peerid);
        self
    }

    pub(crate) fn with_base_url(&mut self, base_url: &'a Url) ->&mut Self {
        self.base_url = Some(base_url);
        self
    }

    pub(crate) fn with_user_identity(&mut self, user: &'a CryptoIdentity) -> &mut Self {
        self.user = Some(user);
        self
    }

    pub(crate) fn with_device_identity(&mut self, device: &'a CryptoIdentity) -> &mut Self {
        self.device = Some(device);
        self
    }

    pub(crate) fn with_access_token(&mut self, _token: &'a str) -> &mut Self {
        //self.access_token = Some(token);
        self
    }

    pub(crate) fn with_access_token_refresh_handler(&mut self, handler: fn(&str)) -> &mut Self {
        self.access_token_refresh_handler = Some(Box::new(handler));
        self
    }

    pub(crate) fn build(&mut self) -> Result<APIClient> {
        if self.peerid.is_none() {
            return Err(Error::Argument("Home peerid is missing".into()));
        };
        if self.user.is_none() {
            return Err(Error::Argument("User identity is missing".into()));
        };
        if self.device.is_none() {
            return Err(Error::Argument("Device identity is missing".into()));
        };

        let Some(base_url) = self.base_url.as_ref() else {
            return Err(Error::Argument("Base url is missing".into()));
        };
        if base_url.scheme() != "http" && base_url.scheme() != "https" {
            return Err(Error::Argument("Invalid base url: scheme must be http or https".into()));
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

    nonce       : Nonce,
}

impl APIClient {
    pub(crate) fn new(b: &mut Builder) -> Result<Self> {
        let client = Client::builder()
            .user_agent("rboson")
            .timeout(Duration::from_secs(30))
            .build().map_err(|e|
                Error::Argument(format!("Creating http client error: {{{e}}}"))
            )?;

        Ok(Self {
            client,
            base_url: b.base_url.unwrap().clone(),
            peerid  : b.peerid  .unwrap().clone(),
            user    : b.user    .unwrap().clone(),
            device  : b.device  .unwrap().clone(),

            access_token: b.access_token.map(|v| v.to_string()),
            access_token_refresh_handler: b.access_token_refresh_handler.take(),

            nonce   : Nonce::random(),
        })
    }

    fn increment_nonce(&mut self) -> Nonce {
        self.nonce.increment();
        self.nonce.clone()
    }

    async fn access_token(&mut self) -> Result<String> {
        if let Some(token) = self.access_token.as_ref() {
            return Ok(token.to_string())
        }

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
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error: {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    Err(Error::State("{e}".into()))?
                },
            }
        };

        let data = rsp.unwrap().json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Deserialize json error: {{{e}}}"))
        })?;

        if let Some(handler) = self.access_token_refresh_handler.as_ref() {
            handler(&data.token);
        }

        self.access_token = Some(data.token);
        Ok(unwrap!(self.access_token).to_string())
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

        let nonce   = self.increment_nonce();
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
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error: {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::CONFLICT) => {
                    warn!("User already exists, trying to refresh access token");
                    self.access_token().await?;
                    return Ok(());
                },
                Some(StatusCode::BAD_REQUEST) | _ => {
                    Err(Error::State("{e}".into()))?
                },
            }
        };

        let token = rsp.unwrap().json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?.token;

        self.access_token = Some(token);
        Ok(())
    }

    pub(crate) async fn register_device(&mut self,
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
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    Err(Error::State("{e}".into()))?
                },
            }
        };

        let data = rsp.unwrap().json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?;

        self.access_token = Some(data.token);
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
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .json(&data)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    Err(Error::State("{e}".into()))?
                },
            }
        };
        let rid = rsp.unwrap().json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?.registrationId;

        Ok(rid)
    }

    pub(crate) async fn finish_register_device_request(&mut self,
        registration_id: &str,
        _timeout: Option<Duration>,
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
        let device_id = self.device.id().clone();
        let sig = self.device.sign_into(nonce.as_bytes()).unwrap();
        let data = RequestData {
            deviceId    : &device_id,
            nonce       : nonce.as_bytes(),
            sig         : sig.as_slice(),
        };

        let path = format!("/api/v1/devices/registrations/{}", registration_id);
        let url = self.base_url.join(path.as_str()).unwrap();
        let rsp = self.client.post(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .bearer_auth(self.access_token().await?)
            .json(&data)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    Err(Error::State("{e}".into()))?
                },
            }
        };
        let rid = rsp.unwrap().json::<ResponseData>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?.registrationId;

        Ok(rid)
    }

    pub(crate) async fn service_info(&mut self) -> Result<MessagingServiceInfo> {
        let url = self.base_url.join("api/v1/service/info").unwrap();
        let rsp = self.client.get(url)
            .header(HTTP_HEADER_CONTENT_TYPE, HTTP_BODY_FORMAT_JSON)
            .bearer_auth(self.access_token().await?)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    return Err(Error::State(format!("{e}")));
                },
            }
        };

        let data = rsp.unwrap().json::<MessagingServiceInfo>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?;

        Ok(data)
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
            .bearer_auth(self.access_token().await?)
            .json(&data)
            .send()
            .await;

        if let Err(e) = rsp.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    Err(Error::State("{e}".into()))?
                },
            }
        };
        Ok(())
    }

    #[allow(unused)]
    pub(crate) async fn get_profile(&mut self, id: &Id) -> Result<profile::Profile> {
        let path = format!("/api/v1/profile/{}", id);
        let url = self.base_url.join(path.as_str()).unwrap();
        let rsp = self.client.get(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .bearer_auth(self.access_token().await?)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    return Err(Error::State(format!("{e}")));
                },
            }
        };

        let data = rsp.unwrap().json::<Profile>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
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
        let path = match version_id {
            Some(id) => format!("/api/v1/contacts/{}", id),
            None => format!("/api/v1/contacts")
        };
        let url = self.base_url.join(path.as_str()).unwrap();
        let rsp = self.client.get(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .bearer_auth(self.access_token().await?)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    return Err(Error::State(format!("{e}")));
                },
            }
        };

        let data = rsp.unwrap().json::<ContactsUpdate>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?;
        Ok(data)
    }

    pub(crate) async fn service_ids(base_url: &Url) -> Result<ServiceIds> {
        let url = base_url.join("/api/v1/service/id").unwrap();
        let result = Client::builder()
            .user_agent("rboson")
            .timeout(Duration::from_secs(30))
            .build();

        let client = result.map_err(|e|
            Error::Argument(format!("Failed to create http client: {e}"))
        )?;

        let rsp = client.get(url)
            .header(HTTP_HEADER_ACCEPT, HTTP_BODY_FORMAT_JSON)
            .send()
            .await;

        if let Err(e) = rsp.as_ref().map_err(|e| {
            Error::State(format!("Sending http request error {e}"))
        })?.error_for_status_ref() {
            match e.status() {
                Some(StatusCode::BAD_REQUEST) | _ => {
                    return Err(Error::State(format!("{e}")));
                },
            }
        };
        let data = rsp.unwrap().json::<JsonServiceIds>().await.map_err(|e| {
            Error::State(format!("Deserializing json error: {e}"))
        })?;
        data.ids()
    }
}

#[derive(Clone, Deserialize)]
pub(crate) struct MessagingServiceInfo {
    #[serde(rename = "peerId")]
    peerid: Id,

    #[serde(rename = "nodeId")]
    nodeid: Id,

    #[serde(rename = "version")]
    version: String,

    #[serde(rename = "endpoints")]
    endpoints: Map<String, serde_json::Value>,

    #[serde(rename = "sslCert")]
    ssl_cert: String,

    #[serde(rename = "features")]
    features: Map<String, serde_json::Value>,
}

impl MessagingServiceInfo {
    pub(crate) fn peerid(&self) -> &Id {
        &self.peerid
    }
}

use std::fmt;
impl fmt::Display for MessagingServiceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessagingServiceInfo {{ peerid: {}, nodeid: {}, version: {}, endpoints: {:?}, ssl_cert: {}, features: {:?} }}",
            self.peerid, self.nodeid, self.version, self.endpoints, self.ssl_cert, self.features)
    }
}
