use std::collections::LinkedList;
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;
use url::Url;
use log::error;

use crate::{
    Id,
    Node,
    signature::KeyPair,
    error::Result,
};

use super::{
    crypto_identity::CryptoIdentity,
    connection_listener::ConnectionListener,
    message_listener::MessageListener,
    channel_listener::ChannelListener,
    contact_listener::ContactListener,
    messaging_client::MessagingClient,
};

#[allow(dead_code)]
pub struct Builder {
    user        : Option<CryptoIdentity>,
    user_name   : Option<String>,
    passphrase  : Option<String>,

    device      : Option<CryptoIdentity>,
    device_node : Option<Arc<Mutex<Node>>>,
    device_name : Option<String>,
    app_name    : Option<String>,

    register_user_and_device: bool,
    register_device : bool,

    peerid      : Option<Id>,
    nodeid      : Option<Id>,
    api_url     : Option<Url>,

    connection_listeners: LinkedList<Box<dyn ConnectionListener>>,
    message_listeners   : LinkedList<Box<dyn MessageListener>>,
    channel_listeners   : LinkedList<Box<dyn ChannelListener>>,
    contact_listeners   : LinkedList<Box<dyn ContactListener>>,
}

impl Builder {
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

            peerid      : None,
            nodeid      : None,
            api_url     : None,

            connection_listeners: LinkedList::new(),
            message_listeners   : LinkedList::new(),
            channel_listeners   : LinkedList::new(),
            contact_listeners   : LinkedList::new(),
        }
    }

    pub fn with_user_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.user = Some(CryptoIdentity::from(keypair));
        self
    }

    pub fn with_user_name(&mut self, name: &str) -> &mut Self {
        self.user_name = Some(name.nfc().collect::<String>());
        self
    }

    pub fn with_device_key(&mut self, keypair: KeyPair) -> &mut Self {
        self.device = Some(CryptoIdentity::from(keypair));
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

    pub fn register_device(&mut self, passphrase: &str) -> &mut Self {
        self.passphrase = Some(passphrase.nfc().collect::<String>());
        self.register_device = true;
        self
    }

    pub fn with_peerid(&mut self, id: Id) -> &mut Self {
        self.peerid = Some(id);
        self
    }

    pub fn with_nodeid(&mut self, id: Id) -> &mut Self {
        self.nodeid = Some(id);
        self
    }

    pub fn with_api_url(&mut self, url: &str) -> &mut Self {
        if let Ok(url) = Url::parse(url) {
            self.api_url = Some(url);
        }
        self
    }

    async fn eligible_check(&self) -> Result<()> {
        unimplemented!()
    }

    pub async fn build(&self) -> Result<Client> {
        if let Err(e) = self.eligible_check().await {
            error!("{e}");
            return Err(e)
        }

        unimplemented!()
    }
}

#[allow(dead_code)]
pub struct Client {
    user:   Id,
    device: Id,
}

#[allow(dead_code)]
impl MessagingClient for Client {
    fn user_id(&self) -> &Id {
        &self.user
    }

    fn device_id(&self) -> &Id {
        &self.device
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
    pub fn new(user: Id, device: Id) -> Self {
        Self { user, device }
    }

    pub fn start(&self) -> Result<()> {
        println!("Started!");
        Ok(())
    }

    pub fn stop(&self) {
        println!("stopped");
    }
}

