use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;
use url::Url;
use log::error;

use crate::{
    Id,
    Node,
    signature::KeyPair,
    error::Result,
    Error,
    Identity,
};

use crate::core::{
    crypto_identity::CryptoIdentity,
};

use super::{
    connection_listener::ConnectionListener,
    message_listener::MessageListener,
    channel_listener::ChannelListener,
    contact_listener::ContactListener,
    messaging_client::MessagingClient,

    user_agent::UserAgent,
};

#[derive(Debug, Default)]
struct MessagingRepository {}

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
    registration_request_handler: Box<dyn Fn(&str, bool)>,

    peerid      : Option<&'a Id>,
    nodeid      : Option<&'a Id>,
    api_url     : Option<Url>,

    repository  : Option<MessagingRepository>,
    repository_db: Option<PathBuf>,

    connection_listeners: Option<Box<dyn ConnectionListener>>,
    message_listeners   : Option<Box<dyn MessageListener>>,
    channel_listeners   : Option<Box<dyn ChannelListener>>,
    contact_listeners   : Option<Box<dyn ContactListener>>,
}

impl<'a> Builder<'a> {
    pub fn new() -> Self {
        Self {
            user        : None,
            user_name   : None,
            passphrase  : None,

            device      : None,
            device_node : None,
            device_name : None,
            app_name    : None,

            register_user_and_device: false,
            register_device : false,

            registration_request_handler: Box::new(|_, _| {}),

            peerid      : None,
            nodeid      : None,
            api_url     : None,

            repository  : None,
            repository_db: None,

            connection_listeners: None,
            message_listeners   : None,
            channel_listeners   : None,
            contact_listeners   : None,
        }
    }

    pub fn with_user_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.user = Some(CryptoIdentity::from_keypair(keypair));
        self
    }

    pub fn with_user_name(&mut self, name: &str) -> &mut Self {
        self.user_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn with_device_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.device = Some(CryptoIdentity::from_keypair(keypair));
        self
    }

    pub fn with_node(&mut self, node: Arc<Mutex<Node>>) -> &mut Self {
        self.device_node = Some(node);
        self
    }

    pub fn with_deivce_name(&mut self, name: &str) -> &mut Self {
        self.device_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn with_app_name(&mut self, name: &str) -> &mut Self {
        self.app_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn register_user_and_device(&mut self, passphrase: &str) -> &mut Self {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_user_and_device = true;
        self
    }

    pub fn register_device(&mut self, passphrase: &str, registration_request_handler: Option<Box<dyn Fn(&str,bool)>>) -> &mut Self  {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_device = true;
        registration_request_handler.map(|handler| {
            self.registration_request_handler = handler;
        });
        self
    }

    pub fn with_peerid(&mut self, id: &'a Id) -> &mut Self {
        self.peerid = Some(id);
        self
    }

    pub fn with_nodeid(&mut self, id: &'a Id) -> &mut Self {
        self.nodeid = Some(id);
        self
    }

    pub fn with_api_url(&mut self, url: &str) -> &mut Self {
        if let Ok(url) = Url::parse(url) {
            self.api_url = Some(url);
        }
        self
    }

    pub fn with_connection_listener(&mut self, listener: Box<dyn ConnectionListener>) -> &mut Self {
        self.connection_listeners = Some(listener);
        self
    }

    pub fn with_message_listener(&mut self, listener: Box<dyn MessageListener>) -> &mut Self {
        self.message_listeners = Some(listener);
        self
    }

    pub fn with_channel_listener(&mut self, listener: Box<dyn ChannelListener>) -> &mut Self {
        self.channel_listeners = Some(listener);
        self
    }

    pub fn with_contact_listener(&mut self, listener: Box<dyn ContactListener>) -> &mut Self {
        self.contact_listeners = Some(listener);
        self
    }

    async fn eligible_check(&self) -> Result<()> {
        //if self.repository.is_none() || self.repository_db.is_none() {
        //    return Err(Error::State("Messaging repository is not configured".into()));
        //}

        //let mut device_check = false;
        //let mut peer_check = false;

        if self.register_user_and_device {
            if self.user.is_none() {
                return Err(Error::State("User key is not configured".into()));
            }
            if self.passphrase.is_none() {
                return Err(Error::State("Passphrase is not configured".into()));
            }

          //  device_check = true;
          //  peer_check = true;
        }
        //unimplemented!()
        Ok(())
    }

    async fn build_default_user_agent(&self) -> Result<UserAgent> {
        UserAgent::new()    // TODO:
    }

    async fn register_agent(&self, _: &UserAgent) -> Result<()> {
        // unimplemented!()
        Ok(())
    }

    pub async fn build(&self) -> Result<Client> {
        self.eligible_check().await.map_err(|e| {
            error!("{e}");
            e
        })?;

        let agent = self.build_default_user_agent().await?;
        self.register_agent(&agent).await?;

        Client::new(self)
    }
}

#[allow(dead_code)]
pub struct Client {
    userid:   Id,
    dev_id: Id,
}

#[allow(dead_code)]
impl MessagingClient for Client {
    fn user_id(&self) -> &Id {
        &self.userid
    }

    fn device_id(&self) -> &Id {
        &self.dev_id
    }

    async fn connect(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn disconnect(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn is_connected(&self) -> bool {
        unimplemented!()
    }

    async fn close(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn revoke_device(&mut self, _device_id: &Id) -> Result<()> {
        unimplemented!()
    }
}

impl Client {
    //pub fn new(user: Id, device: Id) -> Self {
    //    Self { user, device }
    //}
    pub(crate) fn new(b: &Builder) -> Result<Self> {
        Ok(Self {
            userid: b.user.as_ref().unwrap().id().clone(),
            dev_id: b.device.as_ref().unwrap().id().clone(),
        })
    }

    pub fn start(&self) -> Result<()> {
        println!("Started!");
        Ok(())
    }

    pub fn stop(&self) {
        println!("stopped");
    }
}

