use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;
use url::Url;
use log::error;

use crate::{
    unwrap,
    Id,
    PeerInfo,
    signature::KeyPair,
    core::{
        Error,
        Result,
        CryptoIdentity
    },
    dht::Node,
    messaging::{
        ServiceIds,
        UserAgent,
        DefaultUserAgent,
        ConnectionListener,
        MessageListener,
        ContactListener,
        ChannelListener,
        ProfileListener,
        Client,
        api_client::{self, APIClient},
        persistence::database::Database
    }
};

#[allow(dead_code)]
pub struct Builder {
    user                : Option<CryptoIdentity>,
    user_name           : Option<String>,
    passphrase          : Option<String>,

    device              : Option<CryptoIdentity>,
    device_node         : Option<Arc<Mutex<Node>>>,
    device_name         : Option<String>,
    app_name            : Option<String>,

    register_user_and_device: bool,
    register_device     : bool,

    // handler for device registration to acquire user's keypair
    registration_request_handler: Option<Box<dyn Fn(&str) -> Result<bool> + Send + Sync>>,

    messaging_peer      : Option<PeerInfo>,
    messaging_nodeid    : Option<Id>,
    api_url             : Option<Url>,

    repository          : Option<Database>,
    repository_db       : Option<String>,

    connection_listeners: Vec<Box<dyn ConnectionListener>>,
    message_listeners   : Vec<Box<dyn MessageListener>>,
    channel_listeners   : Vec<Box<dyn ChannelListener>>,
    profile_listeners   : Vec<Box<dyn ProfileListener>>,
    contact_listeners   : Vec<Box<dyn ContactListener>>,

    node                : Option<Arc<Mutex<Node>>>,
    user_agent          : Option<Arc<Mutex<dyn UserAgent>>>
}

#[allow(unused)]
impl Builder {
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

            messaging_peer      : None,
            messaging_nodeid    : None,
            api_url             : None,

            repository          : None,
            repository_db       : None,

            connection_listeners: Vec::new(),
            message_listeners   : Vec::new(),
            profile_listeners   : Vec::new(),
            channel_listeners   : Vec::new(),
            contact_listeners   : Vec::new(),

            user_agent          : None,
            node                : None,
        }
    }

    pub fn with_user_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.user = Some(CryptoIdentity::from_keypair(keypair));
        self
    }

    pub fn with_user_private_key(&mut self, sk: &[u8]) -> Result<&mut Self> {
        self.user = Some(CryptoIdentity::from_private_key(sk)?);
        Ok(self)
    }

    pub fn with_new_user_key(&mut self) -> &mut Self {
        self.user = Some(CryptoIdentity::new());
        self
    }

    pub fn user_key(&self) -> Option<&CryptoIdentity> {
        self.user.as_ref()
    }

    pub fn with_user_name(&mut self, name: &str) -> Result<&mut Self> {
        if name.is_empty() {
            return Err(Error::State("User name cannot be empty".into()));
        }
        self.user_name = Some(name.nfc().collect::<String>());
        Ok(self)
    }

    pub fn user_name(&self) -> Option<&str> {
        self.user_name.as_ref().map(|v| v.as_str())
    }

    pub fn with_device_private_key(&mut self, sk: &[u8]) -> Result<&mut Self> {
        self.device = Some(CryptoIdentity::from_private_key(sk)?);
        Ok(self)
    }

    pub fn with_device_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.device = Some(CryptoIdentity::from_keypair(keypair));
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
        self.node = Some(node.clone());
        self
    }

    pub fn with_device_name(&mut self, name: &str) -> Result<&mut Self> {
        if name.is_empty() {
            Err(Error::State("Device name cannot be empty".into()))?;
        }
        self.device_name = Some(name.nfc().collect::<String>());
        Ok(self)
    }

    pub fn device_name(&self) -> Option<&str> {
        self.device_name.as_ref().map(|v| v.as_str())
    }

    pub fn with_app_name(&mut self, name: &str) -> Result<&mut Self> {
        if name.is_empty() {
            Err(Error::State("App name cannot be empty".into()))?;
        }
        self.app_name = Some(name.nfc().collect::<String>());
        Ok(self)
    }

    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_ref().map(|v| v.as_str())
    }

    pub fn with_registering_user(&mut self, passphrase: &str) -> &mut Self {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_user_and_device = true;
        self
    }

    pub fn with_registering_device(&mut self, passphrase: &str) -> &mut Self  {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_device = true;
        self
    }

    pub fn with_device_registering_request_handler(
        &mut self,
        handler: Box<dyn Fn(&str) -> Result<bool> + Send + Sync>
    ) -> &mut Self {
        self.registration_request_handler = Some(handler);
        self.register_device = true;
        self
    }

    pub fn with_messaging_peer(&mut self, peer: PeerInfo) -> Result<&mut Self> {
        self.api_url = peer.alternative_url().map(|url|
                Url::parse(url).map_err(|e|
                    Error::State(format!("Failed to parse API URL: {e}"))
                )
            ).transpose()?;
        self.messaging_peer = Some(peer);
        Ok(self)
    }

    pub fn peer(&self) -> Option<&PeerInfo> {
        assert!(self.messaging_peer.is_some(), "Messaging peer is not set");
        self.messaging_peer.as_ref()
    }

    pub fn peerid(&self) -> &Id {
        assert!(self.messaging_peer.is_some(), "Messaging peer is not set");
        self.messaging_peer.as_ref().unwrap().id()
    }

    /*pub fn with_messaging_nodeid(&mut self, id: &Id) -> &mut Self {
        self.messaging_nodeid = Some(id.clone());
        self
    }
    */

    pub fn nodeid(&self) -> &Id {
        assert!(self.messaging_peer.is_some(), "Messaging peer is not set");
        unwrap!(self.messaging_peer).id()
    }

    pub fn with_api_url<S>(&mut self, url: S) -> Result<&mut Self>
    where
       S: AsRef<str> {

        let url = Url::parse(url.as_ref()).map_err(|e| {
            Error::State(format!("Failed to parse API URL: {e}"))
        })?;

        self.api_url = Some(url);
        Ok(self)
    }

    pub fn api_url(&self) -> &Url {
        assert!(self.api_url.is_some(), "API URL is not set");
        unwrap!(self.api_url)
    }

    pub fn with_messaging_repository(&mut self, path: &str) -> &mut Self {
        self.repository_db = Some(path.to_string());
        self
    }

    pub fn with_connection_listener(
        &mut self,
        listener: impl ConnectionListener + 'static
    ) -> &mut Self {
        self.connection_listeners.push(Box::new(listener));
        self
    }

    pub fn with_connection_listeners(
        &mut self,
        listeners: &mut Vec<Box<dyn ConnectionListener>>
    ) -> &mut Self {
        self.connection_listeners.append(listeners);
        self
    }

    pub fn with_profile_listener(
        &mut self,
        listener: impl ProfileListener + 'static
    ) -> &mut Self {
        self.profile_listeners.push(Box::new(listener));
        self
    }

    pub fn with_profile_listeners(
        &mut self,
        listeners: &mut Vec<Box<dyn ProfileListener>>
    ) -> &mut Self {
        self.profile_listeners.append(listeners);
        self
    }

    pub fn with_message_listener(
        &mut self,
        listener: impl MessageListener + 'static
    ) -> &mut Self {
        self.message_listeners.push(Box::new(listener));
        self
    }

    pub fn with_message_listeners(
        &mut self,
        listeners: &mut Vec<Box<dyn MessageListener>>
    ) -> &mut Self {
        self.message_listeners.append(listeners);
        self
    }

    pub fn with_channel_listener(
        &mut self,
        listener: impl ChannelListener + 'static
    ) -> &mut Self {
        self.channel_listeners.push(Box::new(listener));
        self
    }

    pub fn with_channel_listeners(
        &mut self,
        listeners: &mut Vec<Box<dyn ChannelListener>>
    ) -> &mut Self {
        self.channel_listeners.append(listeners);
        self
    }

    pub fn with_contact_listener(
        &mut self,
        listener: impl ContactListener + 'static
    ) -> &mut Self {
        self.contact_listeners.push(Box::new(listener));
        self
    }

    pub fn with_contact_listeners(
        &mut self,
        listeners: &mut Vec<Box<dyn ContactListener>>
    ) -> &mut Self {
        self.contact_listeners.append(listeners);
        self
    }

    pub(crate) fn with_user_agent<T>(
        &mut self,
        agent: Arc<Mutex<T>>
    ) -> &mut Self
    where
        T: UserAgent + 'static,
    {
        self.user_agent = Some(agent);
        self
    }

    async fn eligible_check(&self) -> Result<()> {
        if self.user_agent.is_some() {
            return Ok(())   // Assuming the userAgent is configured
        }

        if self.repository.is_none() && self.repository_db.is_none() {
            Err(Error::State("Messaging repository is not configured".into()))?;
        }

        let mut device_check = false;
        let mut peer_check = false;

        if self.register_user_and_device {
            if self.user.is_none() {
                Err(Error::State("User key is not configured".into()))?;
            }
            if self.passphrase.is_none() {
                Err(Error::State("Passphrase is not configured".into()))?;
            }
            device_check = true;
            peer_check = true;
        }

        if self.register_device || device_check {
            if self.device.is_none() {
                Err(Error::State("Device key is not configured".into()))?;
            }
            if self.device_name.is_none() {
                Err(Error::State("Device name is not configured".into()))?;
            }
            if self.app_name.is_none() {
                Err(Error::State("App name is not configured".into()))?;
            }
            if self.user.is_some() && self.user_name.is_none() {
                Err(Error::State("User name is not configured".into()))?;
            }
            if self.user.is_none() && self.registration_request_handler.is_none() {
                return Err(Error::State("User registration request handler is not configured".into()));
            }
            peer_check = true;
        }

        if peer_check {
            if self.messaging_peer.is_none() {
                return Err(Error::State("Peer id is not configured".into()));
            }
            if self.api_url.is_none() {
                return Err(Error::State("API URL is not configured".into()));
            }
        }

        Ok(())
    }

    async fn setup_useragent(&mut self) -> Result<Arc<Mutex<dyn UserAgent>>> {
        let Some(agent) = self.user_agent.as_ref() else {
            panic!("User agent is not set");
        };

        let mut locked = agent.lock().unwrap();
        if !locked.is_configured() {
            Err(Error::State("User agent is not configured yet".into()))?;
        }

        while let Some(cb) = self.connection_listeners.pop() {
            locked.add_connection_listener(cb);
        }
        while let Some(cb) = self.profile_listeners.pop() {
            locked.add_profile_listener(cb);
        }
        while let Some(cb) = self.message_listeners.pop() {
            locked.add_message_listener(cb);
        }
        while let Some(cb) = self.channel_listeners.pop() {
            locked.add_channel_listener(cb);
        }
        while let Some(cb) = self.contact_listeners.pop() {
            locked.add_contact_listener(cb);
        }

        Ok(agent.clone())
    }

    async fn build_useragent(&mut self) -> Result<Arc<Mutex<dyn UserAgent>>> {
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
            if agent.user().is_none() {
                agent.set_user(
                    user.clone(),
                    self.user_name.as_ref().map(|v| v.into()).unwrap_or_default()
                );
            } else {
                error!("User is already set in the agent, ignoring the user profile");
            }
        });

        if self.device_node.is_some() && agent.device().is_none() {
            //agent.getDevice().setIdentity(deviceNode);
        }

        self.device.as_ref().map(|device| {
            if agent.device().is_none() {
                _ = agent.set_device(
                    device.clone(),
                    self.device_name.as_ref().map(|v| v.into()).unwrap_or_default(),
                    self.app_name.clone(),
                );
            } else {
                error!("Device is already set in the agent, ignoring the device profile");
            }
        });

        let Some(peer) = self.messaging_peer.as_ref() else {
            return Err(Error::State("Messaging peer is not set".into()));
        };
        agent.set_messaging_peer_info(peer)?;

        while let Some(cb) = self.connection_listeners.pop() {
            agent.add_connection_listener(cb);
        }
        while let Some(cb) = self.profile_listeners.pop() {
            agent.add_profile_listener(cb);
        }
        while let Some(cb) = self.message_listeners.pop() {
            agent.add_message_listener(cb);
        }
        while let Some(cb) = self.channel_listeners.pop() {
            agent.add_channel_listener(cb);
        }
        while let Some(cb) = self.contact_listeners.pop() {
            agent.add_contact_listener(cb);
        }

        Ok(Arc::new(Mutex::new(agent)))
    }

    async fn register_client(&mut self, agent: Arc<Mutex<dyn UserAgent>>) -> Result<()> {
        self.user_agent = Some(agent.clone());

        if !self.register_user_and_device && !self.register_device {
            return Ok(()); // No registration needed
        }

        let mut api_client = api_client::Builder::new()
            .with_base_url(self.api_url())
            .with_home_peerid(self.peerid())
            .with_user_identity(self.user.as_ref().unwrap())
            .with_device_identity(self.device.as_ref().unwrap())
            .build()
            .unwrap();

        let user    = agent.lock().unwrap().user().cloned();
        let device  = agent.lock().unwrap().device().cloned();

        if self.register_user_and_device {
            api_client.register_user_with_device(
                unwrap!(self.passphrase),
                unwrap!(self.user_name),
                unwrap!(self.device_name),
                unwrap!(self.app_name),
            ).await.map_err(|e| {
                error!("Failed to register user and device: {{{e}}}");
                e
            })?;
        }

        if self.register_device {
            if user.is_some() {
                api_client.register_device(
                    unwrap!(self.passphrase),
                    unwrap!(device).name(),
                    unwrap!(device).app_name().unwrap_or("")
                ).await.map(|cred| {
                    //agent.lock().unwrap().on_user_profile_acquired(cred.user());
                    //agent.clone()
                }).map_err(|e| {
                    error!("Failed to register device with error : {{{e}}}");
                    e
                })?;
            } else {
                let rid = api_client.register_device_request(
                    unwrap!(self.device_name),
                    unwrap!(self.app_name)
                ).await.map_err(|e| {
                    error!("Failed to register device request: {e}");
                    e
                })?;

                self.registration_request_handler.as_ref().map(
                    |cb| cb(rid.as_str())
                ).unwrap_or(Ok(true)).map(|finished| {
                    match finished {
                        true => {
                            rid.clone();
                            Ok(())
                        },
                        false => {
                            error!("User cancelled the registration request");
                            Err(Error::State("User cancelled the registration request".into()))
                        }
                    }
                })?.map_err(|e| {
                    error!("Failed to handle registration request: {e}");
                    e
                })?;

                api_client.finish_register_device_request(&rid, None).await.map_err(|e| {
                    error!("Failed to finish device registration request: {e}");
                    e
                })?;
                //agent.lock().unwrap().on_user_profile_acquired(cred.user());
            }
        }
        Ok(())
    }

    pub async fn build(&mut self) -> Result<Client> {
        self.eligible_check().await?;

        let agent = match self.user_agent.is_some() {
            true  => self.setup_useragent().await,
            false => self.build_useragent().await,
        }?;

        self.register_client(agent.clone()).await?;
        agent.lock().unwrap().harden();
        Client::new(self)
    }

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }

    pub(crate) fn user_agent(&self) -> Arc<Mutex<dyn UserAgent>> {
        self.user_agent.as_ref().unwrap().clone()
    }
}
