use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;
use url::Url;
use log::error;

use crate::{
    unwrap,
    Id,
    signature::KeyPair,
    core::Result,
    Error,
    core::crypto_identity::CryptoIdentity,
    dht::Node,
};

use crate::messaging::{
    ServiceIds,
    UserAgent,
    DefaultUserAgent,
    ConnectionListener,
    MessageListener,
    ContactListener,
    ChannelListener,
    ProfileListener,
    Client,
};

use super::{
    api_client::{self, APIClient},
    persistence::database::Database,
};

#[allow(dead_code)]
pub struct Builder<'a> {
    user        : Option<CryptoIdentity>,
    user_name   : Option<String>,
    passphrase  : Option<String>,

    device      : Option<CryptoIdentity>,
    device_node : Option<Arc<Mutex<Node>>>,
    device_name : Option<String>,
    app_name    : Option<String>,

    register_user_and_device: bool,
    register_device : bool,

    // handler for device registration to acquire user's keypair
    registration_request_handler: Option<Box<dyn Fn(&str, bool) + Send + Sync>>,

    peerid      : Option<&'a Id>,
    nodeid      : Option<&'a Id>,
    api_url     : Option<Url>,

    repository  : Option<Database>,
    repository_db: Option<&'a str>,

    connection_listener : Option<Box<dyn ConnectionListener>>,
    message_listener    : Option<Box<dyn MessageListener>>,
    channel_listener    : Option<Box<dyn ChannelListener>>,
    profile_listener    : Option<Box<dyn ProfileListener>>,
    contact_listener    : Option<Box<dyn ContactListener>>,

    user_agent  : Option<Arc<Mutex<DefaultUserAgent>>>
}

#[allow(unused)]
impl<'a> Builder<'a> {
    pub fn new() -> Self {
        Self {
            user                : None,
            user_name           : None,
            passphrase          : None,

            device              : None,
            device_node         : None,
            device_name         : None,
            app_name            : None,

            register_user_and_device    : false,
            register_device     : false,

            registration_request_handler: None,

            peerid              : None,
            nodeid              : None,
            api_url             : None,

            repository          : None,
            repository_db       : None,

            connection_listener : None,
            message_listener    : None,
            profile_listener    : None,
            channel_listener    : None,
            contact_listener    : None,

            user_agent          : None,
        }
    }

    pub fn with_user_key(&mut self, keypair: &KeyPair) -> &mut Self {
        self.user = Some(CryptoIdentity::from_keypair(keypair.clone()));
        self
    }

    pub fn with_new_user_key(&mut self) -> &mut Self {
        self.user = Some(CryptoIdentity::new());
        self
    }

    pub fn user_key(&self) -> Option<&CryptoIdentity> {
        self.user.as_ref()
    }

    pub fn with_user_name(&mut self, name: &str) -> &mut Self {
        self.user_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn user_name(&self) -> Option<&str> {
        self.user_name.as_ref().map(|v| v.as_str())
    }

    pub fn with_device_key(&mut self, keypair: &KeyPair) -> &mut Self {
        self.device = Some(CryptoIdentity::from_keypair(keypair.clone()));
        self
    }

    pub fn with_new_device_key(&mut self) -> &mut Self {
        self.device = Some(CryptoIdentity::new());
        self
    }

    pub fn device_key(&self) -> Option<&CryptoIdentity> {
        self.device.as_ref()
    }

    pub fn with_device_node(&mut self, node: Arc<Mutex<Node>>) -> &mut Self {
        unimplemented!()
    }

    pub fn with_device_name(&mut self, name: &str) -> &mut Self {
        self.device_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn device_name(&self) -> Option<&str> {
        self.device_name.as_ref().map(|v| v.as_str())
    }

    pub fn with_app_name(&mut self, name: &str) -> &mut Self {
        self.app_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_ref().map(|v| v.as_str())
    }

    pub fn register_user_and_device(&mut self, passphrase: &str) -> &mut Self {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_user_and_device = true;
        self
    }

    pub fn register_device(&mut self, passphrase: &str) -> &mut Self  {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_device = true;
        self
    }

    pub fn register_device_registration_request_handler(&mut self, handler: Box<dyn Fn(&str,bool) + Send + Sync>) -> &mut Self {
        self.registration_request_handler = Some(handler);
        self.register_device = true;
        self
    }

    pub fn with_peerid(&mut self, id: &'a Id) -> &mut Self {
        self.peerid = Some(id);
        self
    }

    pub fn peerid(&self) -> Option<&Id> {
        self.peerid
    }

    // TODO: do we need this API ?
    pub fn with_nodeid(&mut self, id: &'a Id) -> &mut Self {
        self.nodeid = Some(id);
        self
    }

    pub fn nodeid(&self) -> Option<&Id> {
        self.nodeid
    }

    pub fn with_api_url(&mut self, url: &str) -> &mut Self {
        if let Ok(url) = Url::parse(url) {
            self.api_url = Some(url);
        }
        self
    }

    pub fn api_url(&self) -> Option<&Url> {
        self.api_url.as_ref()
    }

    pub fn with_messaging_repository(&mut self, path: &'a str) -> &mut Self {
        self.repository_db = Some(path);
        self
    }

    pub fn with_connection_listener(&mut self, listener: impl ConnectionListener + 'static) -> &mut Self {
        self.connection_listener = Some(Box::new(listener));
        self
    }

    pub fn with_profile_listener(&mut self, listener: impl ProfileListener + 'static) -> &mut Self {
        self.profile_listener = Some(Box::new(listener));
        self
    }

    pub fn with_message_listener(&mut self, listener: impl MessageListener + 'static ) -> &mut Self {
        self.message_listener = Some(Box::new(listener));
        self
    }

    pub fn with_channel_listener(&mut self, listener: impl ChannelListener + 'static ) -> &mut Self {
        self.channel_listener = Some(Box::new(listener));
        self
    }

    pub fn with_contact_listener(&mut self, listener: impl ContactListener + 'static) -> &mut Self {
        self.contact_listener = Some(Box::new(listener));
        self
    }

    pub(crate) fn with_user_agent(&mut self, agent: Arc<Mutex<DefaultUserAgent>>) -> &mut Self {
        self.user_agent = Some(agent);
        self
    }

    async fn eligible_check(&self) -> Result<()> {
        if self.repository.is_none() || self.repository_db.is_none() {
            return Err(Error::State("Messaging repository is not configured".into()));
        }

        let mut device_check = false;
        let mut peer_check = false;

        if self.register_user_and_device {
            if self.user.is_none() {
                return Err(Error::State("User key is not configured".into()));
            }
            if self.passphrase.is_none() {
                return Err(Error::State("Passphrase is not configured".into()));
            }

            device_check = true;
            peer_check = true;
        }

        if self.register_device || device_check {
            if self.device.is_none() {
                return Err(Error::State("Device key is not configured".into()));
            }
            if self.device_name.is_none() {
                return Err(Error::State("Device name is not configured".into()));
            }
            if self.app_name.is_none() {
                return Err(Error::State("App name is not configured".into()));
            }
            if self.user.is_some() && self.user_name.is_none() {
                return Err(Error::State("User name is not configured".into()));
            }
            if self.user.is_none() && self.registration_request_handler.is_none() {
                return Err(Error::State("User registration request handler is set".into()));
            }
            peer_check = true;
        }

        if peer_check {
            if self.peerid.is_none() {
                return Err(Error::State("Peer ID is not configured".into()));
            }
            if self.device_node.is_none() &&  self.api_url.is_none() {
                return Err(Error::State("API URL is not configured".into()));
            }
        } else {
            if self.peerid.is_some() {
                if self.device_node.is_none() && self.api_url.is_none() {
                    return Err(Error::State("API URL is not configured".into()));
                }
            } else {
                if self.api_url.is_some() {
                    return Err(Error::State("Peer id is not configured".into()));
                }
            }
        }
        Ok(())
    }

    async fn setup_user_agent(&mut self) -> Result<Arc<Mutex<dyn UserAgent>>>  {
        let Some(agent) = self.user_agent.as_ref() else {
            return Err(Error::State("User agent is not set up yet".into()));
        };

        let mut agent_guard = agent.lock().unwrap();
        if !agent_guard.is_configured() {
            return Err(Error::State("User agent is not configured yet".into()));
        }
        self.connection_listener.take().map(|v|{
            agent_guard.set_connection_listener(v);
        });
        self.message_listener.take().map(|v| {
            agent_guard.set_message_listener(v);
        });
        self.channel_listener.take().map(|v| {
            agent_guard.set_channel_listener(v);
        });
        self.contact_listener.take().map(|v| {
            agent_guard.set_contact_listener(v);
        });

        return Ok(agent.clone())
    }

    async fn build_default_user_agent(&mut self) -> Result<Arc<Mutex<dyn UserAgent>>> {
        let mut agent = DefaultUserAgent::new(None)?;
        let repos = match self.repository.take() {
            Some(r) => r,
            None => {
                let path = PathBuf::from(self.repository_db.as_ref().unwrap());
                let db = Database::open(&path).map_err(|e| {
                    Error::State(format!("Access the messaging repository failed: {e}"))
                })?;
                // TODO: agent.set_messaging_repository(&db);
                db
            }
        };
        self.repository = Some(repos);

        self.user.as_ref().map(|user| {
            agent.set_user(user.clone(), self.user_name.as_ref().map(|v|v.into()).unwrap())
        });

        /*
        if (deviceNode != null && agent.getDevice() != null)
            agent.getDevice().setIdentity(deviceNode);

        if (device != null) {
            if (agent.getDevice() == null)
                agent.setDevice(device, deviceName, appName);
            else
                log.warn("Messaging repository is configured, device profile will be ignored");
        }
         */

        if let Some(listener) = self.connection_listener.take() {
            agent.set_connection_listener(listener);
        }
        if let Some(listener) = self.message_listener.take() {
            agent.set_message_listener(listener);
        }
        if let Some(listener) = self.channel_listener.take() {
            agent.set_channel_listener(listener);
        }
        if let Some(listener) = self.contact_listener.take() {
            agent.set_contact_listener(listener);
        }
        Ok(Arc::new(Mutex::new(agent)))
    }

    async fn register_agent(&self, _: Arc<Mutex<dyn UserAgent>>) -> Result<()> {
        let mut api_client = api_client::Builder::new()
            .with_base_url(self.api_url.as_ref().unwrap().as_str())
            .with_home_peerid(self.peerid.as_ref().unwrap())
            .with_user_identity(self.user.as_ref().unwrap())
            .with_device_identity(self.device.as_ref().unwrap())
            .build()
            .unwrap();

        let user = unwrap!(self.user_agent).lock().unwrap().user();
        let device = unwrap!(self.user_agent).lock().unwrap().device();

        if self.register_user_and_device {
            api_client.register_user_with_device(
                self.passphrase.as_ref().unwrap(),
                self.user_name.as_ref().unwrap(),
                self.device_name.as_ref().unwrap(),
                self.app_name.as_ref().unwrap(),
            ).await?;
        }

        Ok(())
    }

    pub async fn build(&mut self) -> Result<Client> {
        self.eligible_check().await.map_err(|e| {
            error!("{e}");
            e
        })?;

        let agent = match self.user_agent.is_some() {
            true => self.setup_user_agent().await,
            false => self.build_default_user_agent().await
        }?;

        self.register_agent(agent).await?;
        Client::new(self)
    }

    pub(crate) fn user_agent(&self) -> Option<Arc<Mutex<DefaultUserAgent>>> {
        self.user_agent.as_ref().map(|v| v.clone())
    }

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }
}
