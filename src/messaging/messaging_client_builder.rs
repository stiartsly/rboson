use std::sync::{Arc, Mutex};
//use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;
use url::Url;
use log::{warn, error};

use crate::{
    PeerInfo,
    NodeInfo,
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
        UserAgentCaps,
        ConnectionListener,
        MessageListener,
        ContactListener,
        ChannelListener,
        ProfileListener,
        MessagingClient,
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
    device_name         : Option<String>,
    device_node         : Option<Arc<Mutex<Node>>>,
    app_name            : Option<String>,

    register_user_and_device    : bool,
    register_device_only        : bool,

    // handler for device registration to acquire user's keypair
    register_request_handler    : Option<Box<dyn Fn(&str) -> Result<bool> + Send + Sync>>,

    api_url             : Option<Url>,
    messaging_peer      : Option<PeerInfo>,
    messaging_node      : Option<NodeInfo>,

    repository          : Option<Database>,
    repository_db       : Option<String>,

    connection_listener : Option<Box<dyn ConnectionListener>>,
    message_listener    : Option<Box<dyn MessageListener>>,
    channel_listener    : Option<Box<dyn ChannelListener>>,
    profile_listener    : Option<Box<dyn ProfileListener>>,
    contact_listener    : Option<Box<dyn ContactListener>>,

    node                : Option<Arc<Mutex<Node>>>,
    ua                  : Option<Arc<Mutex<UserAgent>>>
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
            register_device_only        : false,
            register_request_handler    : None,

            api_url             : None,
            messaging_peer      : None,
            messaging_node      : None,

            repository          : None,
            repository_db       : None,

            connection_listener : None,
            message_listener    : None,
            profile_listener    : None,
            channel_listener    : None,
            contact_listener    : None,

            node                : None,
            ua                  : None
        }
    }

    pub fn with_new_user_key(&mut self) -> &mut Self {
        self.user = Some(CryptoIdentity::new());
        self
    }

    pub fn with_user_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.user = Some(CryptoIdentity::from_keypair(keypair));
        self
    }

    pub fn with_user_key_from_private_key(&mut self, private_key: &[u8]) -> Result<&mut Self> {
        self.user = Some(CryptoIdentity::from_private_key(private_key)?);
        Ok(self)
    }

    pub fn with_user_name(&mut self, name: &str) -> &mut Self {
        self.user_name = match !name.is_empty() {
            true  => Some(name.nfc().collect::<String>()),
            false => None,
        };
        self
    }

    pub fn with_new_device_key(&mut self) -> &mut Self {
        self.device = Some(CryptoIdentity::new());
        self
    }

    pub fn with_device_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.device = Some(CryptoIdentity::from_keypair(keypair));
        self
    }

    pub fn with_device_key_from_private_key(&mut self, private_key: &[u8]) -> Result<&mut Self> {
        self.device = Some(CryptoIdentity::from_private_key(private_key)?);
        Ok(self)
    }

    pub fn with_device_name(&mut self, name: &str) -> &mut Self {
        self.device_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn with_app_name(&mut self, name: &str) -> &mut Self {
        self.app_name = match !name.is_empty() {
            true  => Some(name.nfc().collect::<String>()),
            false => None,
        };
        self
    }

    pub fn with_device_node(&mut self, node: Arc<Mutex<Node>>) -> &mut Self {
        self.node = Some(node.clone());
        self
    }

    pub fn register_user_and_device(&mut self, passphrase: &str) -> &mut Self {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_user_and_device = true;
        self
    }

    pub fn register_device(&mut self, passphrase: &str) -> &mut Self  {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_device_only = true;
        self
    }

    pub fn with_device_registeration_request_handler(
        &mut self,
        handler: Box<dyn Fn(&str) -> Result<bool> + Send + Sync>
    ) -> &mut Self {
        self.register_request_handler = Some(handler);
        self.register_device_only = true;
        self
    }

    pub fn with_messaging_peer(&mut self, peer: PeerInfo) -> Result<&mut Self> {
        self.messaging_peer = Some(peer);
        Ok(self)
    }

    pub fn with_messaging_node(&mut self, node: NodeInfo) -> Result<&mut Self> {
        self.messaging_node = Some(node);
        Ok(self)
    }

    pub fn with_api_url(&mut self, url: &str) -> Result<&mut Self> {
        let api_url = Url::parse(&url).map_err(|e|
            Error::State(format!("Error parsing API URL: {e}"))
        )?;

        self.api_url = Some(api_url.clone());
        Ok(self)
    }

    pub fn with_messaging_repository(&mut self, path: &str) -> &mut Self {
        self.repository_db = Some(path.to_string());
        self
    }

    pub fn with_connection_listener(&mut self,
        listener: impl ConnectionListener + 'static
    ) -> &mut Self {
        self.connection_listener = Some(Box::new(listener));
        self
    }

    pub fn with_profile_listener(&mut self,
        listener: impl ProfileListener + 'static
    ) -> &mut Self {
        self.profile_listener = Some(Box::new(listener));
        self
    }

    pub fn with_message_listener(&mut self,
        listener: impl MessageListener + 'static
    ) -> &mut Self {
        self.message_listener = Some(Box::new(listener));
        self
    }

    pub fn with_channel_listener(&mut self,
        listener: impl ChannelListener + 'static
    ) -> &mut Self {
        self.channel_listener = Some(Box::new(listener));
        self
    }

    pub fn with_contact_listener(&mut self,
        listener: impl ContactListener + 'static
    ) -> &mut Self {
        self.contact_listener = Some(Box::new(listener));
        self
    }

    pub(crate) fn with_user_agent(&mut self,
        agent: Arc<Mutex<UserAgent>>
    ) -> &mut Self {
        self.ua = Some(agent);
        self
    }

    async fn eligible_check(&mut self) -> Result<()> {
        if self.ua.is_some() {
            return Ok(());  // User agent is already set, skip checks
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

        if self.register_device_only || device_check {
            if self.device.is_none() {
                Err(Error::State("Device key is not configured".into()))?;
            }
            if self.device_name.is_none() {
                Err(Error::State("Device name is not configured".into()))?;
            }
            if self.app_name.is_none() {
                Err(Error::State("App name is not configured".into()))?;
            }
            if self.user.is_some() && self.passphrase.is_none() {
                Err(Error::State("Passphrase is not configured".into()))?;
            }
            if self.user.is_none() && self.register_request_handler.is_none() {
                Err(Error::State("Registration request handler is not configured".into()))?;
            }
            peer_check = true;
        }

        if peer_check {
            let Some(peer) = self.messaging_peer.as_ref() else {
                return Err(Error::State("Peer is not configured".into()));
            };

            if self.api_url.is_some() {
                return Ok(());
            }

            let url = match peer.alternative_url().as_ref() {
                Some(url) => Url::parse(url),
                None =>  {
                    let Some(node) = self.messaging_node.as_ref() else {
                        return Err(Error::State(
                            "Messaging node is not configured to derive API URL".into()
                        ));
                    };
                    Url::parse(
                        &format!("http://{}:{}",
                            node.socket_addr().ip(),
                            peer.port()
                        )
                    )
                }
            }.map_err(|e| {
                Error::State(format!("Error parsing API URL: {e}"))
            })?;
            self.api_url = Some(url);
        }
        Ok(())
    }

    async fn setup_user_agent(&mut self) -> Result<(Arc<Mutex<UserAgent>>)> {
        let Some(ua) = self.ua.clone() else {
            return Err(Error::State("User agent is not set".into()));
        };
        if !crate::lock!(ua).is_configured() {
            Err(Error::State("User agent is not configured yet".into()))?;
        }

        if let Some(cb) = self.connection_listener.take() {
            crate::lock!(ua).add_connection_listener(cb);
        }
        if let Some(cb) = self.profile_listener.take() {
            crate::lock!(ua).add_profile_listener(cb);
        }
        if let Some(cb) = self.message_listener.take() {
            crate::lock!(ua).add_message_listener(cb);
        }
        if let Some(cb) = self.channel_listener.take() {
            crate::lock!(ua).add_channel_listener(cb);
        }
        if let Some(cb) = self.contact_listener.take() {
            crate::lock!(ua).add_contact_listener(cb);
        }
        Ok(ua)
    }

    async fn build_user_agent(&mut self) -> Result<Arc<Mutex<UserAgent>>> {
        let mut ua = UserAgent::new(None);
        /*
        ua.set_repository(
            match self.repository.take() {
                Some(r) => r,
                None => {
                    let path = PathBuf::from(crate::unwrap!(self.repository_db));
                    Database::open(&path).map_err(|e| {
                        Error::State(format!("Error accessing the messaging repository: {e}"))
                    })?
                }
            }
        )?;
        */

        if let Some(user) = self.user.as_ref() {
            if ua.user().is_none() {
                ua.set_user(user.clone(), self.user_name.as_deref())?;
            } else {
                warn!("User is already set in the user agent, ignoring user profile");
            }
        }

        if self.device_node.is_some() && ua.device().is_some() {
            //ua.device_mut().set_identity(
            //    crate::unwrap!(self.device_node).clone()
            //);
        }

        if let Some(device) = self.device.as_ref() {
            if ua.device().is_none() {
                ua.set_device(device.clone(),
                    self.device_name.as_ref().unwrap(),
                    self.app_name.as_ref().unwrap())?
            } else {
                warn!("Device is already set in the user agent, ignoring device profile");
            }
        };

        let Some(peer) = self.messaging_peer.as_ref() else {
            return Err(Error::State("Messaging peer is not set".into()));
        };

        ua.set_messaging_peer_info(peer)?;

        if let Some(cb) = self.connection_listener.take() {
            ua.add_connection_listener(cb);
        }
        if let Some(cb) = self.profile_listener.take() {
            ua.add_profile_listener(cb);
        }
        if let Some(cb) = self.message_listener.take() {
            ua.add_message_listener(cb);
        }
        if let Some(cb) = self.channel_listener.take() {
            ua.add_channel_listener(cb);
        }
        if let Some(cb) = self.contact_listener.take() {
            ua.add_contact_listener(cb);
        }

        Ok(Arc::new(Mutex::new(ua)))
    }

    async fn register_client(&mut self, ua: Arc<Mutex<UserAgent>>) -> Result<()> {
        self.ua = Some(ua.clone());

        if !self.register_user_and_device && !self.register_device_only {
            return Ok(()); // No registration required.
        }

        let mut api_client = api_client::Builder::new()
            .with_base_url(self.api_url())
            .with_home_peerid(self.peer().id())
            .with_user_identity(self.user.as_ref().unwrap())
            .with_device_identity(self.device.as_ref().unwrap())
            .build()?;

        let user = crate::lock!(ua).user().cloned();
        let device = crate::lock!(ua).device().cloned();

        if self.register_user_and_device {
            println!(">>>> Registering user with device");
            api_client.register_user_with_device(
                crate::unwrap!(self.passphrase),
                crate::unwrap!(self.user_name),
                crate::unwrap!(self.device_name),
                crate::unwrap!(self.app_name),
            ).await.map_err(|e| {
                error!("Failed to register user and device: {e}");
                e
            })?;
        }

        if !self.register_device_only {
            return Ok(());
        }

        if user.is_some() {
            println!(">>>> Registering device with user");
            let cred = api_client.register_device_with_user(
                crate::unwrap!(self.passphrase),
                crate::unwrap!(device).name(),
                crate::unwrap!(device).app_name()
            ).await?;

            crate::lock!(ua).on_user_profile_acquired(&cred);
        } else {
            let rid = api_client.register_device_request(
                crate::unwrap!(self.device_name),
                crate::unwrap!(self.app_name)
            ).await?;  // return registeration ID if success

            // TODO:
            self.register_request_handler.as_ref().map(
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
            }).map_err(|e| {
                error!("Failed to handle registration request: {e}");
                e
            })?;

            api_client.finish_register_device_request(&rid, None).await?;
            //crate::lock!(ua).on_user_profile_acquired(cred.user());
        }
        Ok(())
    }

    pub async fn build_into(&mut self) -> Result<MessagingClient> {
        self.eligible_check().await?;

        let ua = match self.ua.is_some() {
            true  => self.setup_user_agent().await?,
            false => self.build_user_agent().await?,
        };

        self.register_client(ua).await?;
        MessagingClient::new(&self)
    }

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }

    pub(crate) fn ua(&self) -> Arc<Mutex<UserAgent>> {
        self.ua.as_ref().expect("User agent is not set").clone()
    }

    pub(crate) fn peer(&self) -> &PeerInfo {
        self.messaging_peer
            .as_ref()
            .expect("Messaging peer is not set")
    }

    pub(crate) fn api_url(&self) -> &Url {
        self.api_url.as_ref().expect("API URL is not set")
    }
}
