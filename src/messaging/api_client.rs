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

#[derive(Serialize, Debug)]
#[allow(non_snake_case)]
struct RefreshAccessTokenReqData {
    userId  : String,
    deviceId: String,
    nonce   : Vec<u8>,
    userSig : Vec<u8>,
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
    nonce   : Vec<u8>,
    userSig : Vec<u8>,
    deviceSig   : Vec<u8>,
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
    peerid      : Id,
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
    pub(crate) fn new(peerid: &Id, base_url: &str) -> Self {
        Self {
            peerid  : peerid.clone(),
            base_url: Url::parse(base_url).unwrap(),
            client  : Client::builder().build().unwrap(),

            user    : None,
            device  : None,
            access_token: None,

            access_token_refresh_handler: None,

            nonce   : RefCell::new(Nonce::random())
        }
    }

    pub(crate) fn with_user_identity(&mut self, user: &CryptoIdentity) -> &Self {
        self.user = Some(user.clone());
        self
    }

    pub(crate) fn with_device_identity(&mut self, _device: &CryptoIdentity) -> &Self {
        self.device = Some(_device.clone());
        self
    }

    pub(crate) fn with_access_token(&mut self, access_token: &str) -> &Self {
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

        let rsp = result.map_err(|e| {
            Error::State(format!("Http error: sending http request error {e}"))
        })?.error_for_status().map_err(|e|
            Error::State(format!("Http error: invalid http response {e}"))
        )?;

        let data = rsp.json::<ServiceIdsRspData>().await.map_err(|e| {
            Error::State(format!("Http error: deserialize json error {e}"))
        })?;

        let Some(peerid) = data.peerId else {
            return Err(Error::State("Http error: missing peer id".into()));
        };
        let Ok(peerid) = Id::try_from(peerid.as_str()) else {
            return Err(Error::State("Http error: invalid peer id".into()));
        };

        let Some(nodeid) = data.nodeId else {
            return Err(Error::State("Http error: missing nodeid id".into()));
        };
        let Ok(nodeid) = Id::try_from(nodeid.as_str()) else {
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
            userId  : user.id().to_string(),
            deviceId: device.id().to_string(),
            nonce   : nonce.borrow().as_bytes().to_vec(),
            userSig : user.sign_into(nonce.borrow().as_bytes()).unwrap(),
            deviceSig: device.sign_into(nonce.borrow().as_bytes()).unwrap()
        };

        let url = self.base_url.join("/api/v1/auth").unwrap();
        println!("url: {}", url);
        println!("data: {:?}", data);
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
        // byte[] profileDigest = Profile.digest(user.getId(), homePeerId, userName, false, null);
        let profile_digest = vec![0u8; 0]; // TODO:
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

#[tokio::test]
async fn test_refresh_access_token() {
    use crate::signature;
    let peerid:Id = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ".try_into().unwrap();
    let mut client = APIClient::new(&peerid, "http://155.138.245.211:8882");

    let user = CryptoIdentity::from_keypair(signature::KeyPair::random());
    let device = CryptoIdentity::from_keypair(signature::KeyPair::random());
    client.with_user_identity(&user);
    client.with_device_identity(&device);

    let result = client.refresh_access_token().await;
    println!("result: {:?}", result);
}
