use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use unicode_normalization::UnicodeNormalization;
use url::Url;
use log::{warn, error};

use crate::{
    Id,
    Node,
    signature::KeyPair,
    error::Result,
    Error,
    Identity,
    ServiceIds
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
    api_client::{self, APIClient},

    user_agent::{UserAgent, DefaultUserAgent},
    persistence::database::Database,
    client_device::ClientDevice,
    channel::{Role, Permission, Channel},
    invite_ticket::InviteTicket,
    contact::Contact,
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
    registration_request_handler: Box<dyn Fn(&str, bool)>,

    peerid      : Option<&'a Id>,
    nodeid      : Option<&'a Id>,
    api_url     : Option<Url>,

    repository  : Option<Rc<RefCell<Database>>>,
    repository_db: Option<&'a str>,

    connection_listeners: Option<Box<dyn ConnectionListener>>,
    message_listeners   : Option<Box<dyn MessageListener>>,
    channel_listeners   : Option<Box<dyn ChannelListener>>,
    contact_listeners   : Option<Box<dyn ContactListener>>,

    user_agent  : Option<Rc<RefCell<dyn UserAgent>>>
}

#[allow(unused)]
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

            user_agent  : None,
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

    pub fn with_messaging_repository(&mut self, path: &'a str) -> &mut Self {
        self.repository_db = Some(path);
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

    pub(crate) fn with_user_agent(&mut self, agent: Rc<RefCell<dyn UserAgent>>) -> &mut Self {
        self.user_agent = Some(agent);
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

    async fn build_default_user_agent(&mut self) -> Result<Rc<RefCell<dyn UserAgent>>> {
        let agent = Rc::new(RefCell::new(DefaultUserAgent::new(None).unwrap()));
        let repo = match self.repository.take() {
            Some(r) => r,
            None => {
                self.repository_db.as_ref().map(|db|
                    println!("db path: {}", db)
                );
                if self.repository_db.is_none() {
                    println!("db path is None");
                }

                let path = PathBuf::from(self.repository_db.as_ref().unwrap());
                let db = Database::open(&path).map_err(|e| {
                    error!("{e}");
                    Error::State("Messaging repository is not configured".into())
                })?;
                Rc::new(RefCell::new(db))
            }
        };
        self.repository = Some(repo);

        if let Some(user) = self.user.as_ref() {
            if agent.borrow().user().is_none() {
                agent.borrow_mut().set_user(user, self.user_name.as_deref().unwrap());
            } else {
                warn!("Messaging repository is configured, user profile will be ignored.");
            }
        }
        Ok(agent)
    }

    async fn register_agent(&self, _: Rc<RefCell<dyn UserAgent>>) -> Result<()> {
        let mut api_client = api_client::Builder::new()
            .with_base_url(self.api_url.as_ref().unwrap().as_str())
            .with_home_peerid(self.peerid.as_ref().unwrap())
            .with_user_identity(self.user.as_ref().unwrap())
            .with_device_identity(self.device.as_ref().unwrap())
            .build()
            .unwrap();

        let user = self.user_agent.as_ref().unwrap().borrow().user();
        let device = self.user_agent.as_ref().unwrap().borrow().device();

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

    async fn setup_user_agent(&mut self) -> Result<Rc<RefCell<dyn UserAgent>>>  {
        let Some(agent) = self.user_agent.take() else {
            return Err(Error::State("User agent is not set up yet".into()));
        };

        if !agent.borrow().is_configured() {
            return Err(Error::State("User agent is not configured yet".into()));
        }

        /* TODO: Listener */
        return Ok(agent)
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

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }
}

#[allow(dead_code)]
pub struct Client {
    userid:   Id,
    dev_id: Id,

    api_client: APIClient,
}

impl Client {
    pub(crate) fn new(b: &Builder) -> Result<Self> {
        Ok(Self {
            userid: b.user.as_ref().unwrap().id().clone(),
            dev_id: b.device.as_ref().unwrap().id().clone(),

            api_client: api_client::Builder::new()
                .with_base_url(b.api_url.as_ref().unwrap().as_str())
                .with_home_peerid(b.peerid.as_ref().unwrap())
                .with_user_identity(b.user.as_ref().unwrap())
                .with_device_identity(b.device.as_ref().unwrap())
                .build()
                .unwrap(),
        })
    }

    pub fn start(&self) -> Result<()> {
        println!("Messaging client Started!");
        Ok(())
    }

    pub fn stop(&self) {
        println!("Messaging client stopped");
    }

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }
}

#[allow(dead_code)]
impl MessagingClient for Client {
    fn userid(&self) -> &Id {
        &self.userid
    }

    fn user_agent(&self) -> &Box<dyn UserAgent> {
        unimplemented!()
    }

    async fn close(&mut self) -> Result<()> {
        unimplemented!()
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

    async fn update_profile(&mut self, name: &str, avatar: bool) -> Result<()> {
        let name = name.nfc().collect::<String>();
        self.api_client.update_profile(&name, avatar).await
    }

    async fn upload_avatar(&mut self, content_type: &str, avatar: &[u8]) -> Result<String> {
        self.api_client.upload_avatar(content_type, avatar).await
    }

    async fn upload_avatar_with_filename(&mut self, content_type: &str, file_name: &str) -> Result<String> {
        self.api_client.upload_avatar_with_filename(
            content_type,
            file_name.into()
        ).await
    }

    async fn devices(&self) -> Result<Vec<ClientDevice>> {
        unimplemented!()
    }

    async fn revoke_device(&mut self, _device_id: &Id) -> Result<()> {
        unimplemented!()
    }

    async fn create_channel(&mut self, _name: &str, _notice: Option<&str>) -> Result<Channel> {
        unimplemented!()
    }

    async fn create_channel_with_permission(&mut self, _permission: &Permission, _name: &str, _notice: Option<&str>) -> Result<Channel> {
        unimplemented!()
    }

    async fn remove_channel(&mut self, _channel_id: &Id) -> Result<()> {
        unimplemented!()
    }

    async fn join_channel(&mut self, _ticket: &InviteTicket) -> Result<()> {
        unimplemented!()
    }

    async fn leave_channel(&mut self, _channel_id: &Id) -> Result<()> {
        unimplemented!()
    }

    async fn create_invite_ticket(&mut self, _channel_id: &Id) -> Result<InviteTicket> {
        unimplemented!()
    }

    async fn create_invite_ticket_with_invitee(&mut self, _channel_id: &Id, _invitee: &Id) -> Result<InviteTicket> {
        unimplemented!()
    }

    async fn set_channel_owner(&mut self, _channel_id: &Id, _new_owner: &Id) -> Result<()> {
        unimplemented!()
    }

    async fn set_channel_permission(&mut self, _channel_id: &Id, _permission: &Permission) -> Result<()> {
        unimplemented!()
    }

    async fn set_channel_name(&mut self, _channel_id: &Id, _name: &str) -> Result<()> {
        unimplemented!()
    }

    async fn set_channel_notice(&mut self, _channel_id: &Id, _notice: &str) -> Result<()> {
        unimplemented!()
    }

    async fn set_channel_member_role(&mut self, _channel_id: &Id, _members: Vec<&Id>, _role: &Role) -> Result<()> {
        unimplemented!()
    }

    async fn ban_channel_members(&mut self, _channel_id: &Id, _members: Vec<&Id>) -> Result<()> {
        unimplemented!()
    }

    async fn unban_channel_members(&mut self, _channel_id: &Id, _members: Vec<&Id>) -> Result<()> {
        unimplemented!()
    }

    async fn remove_channel_members(&mut self, _channel_id: &Id, _members: Vec<&Id>) -> Result<()> {
        unimplemented!()
    }

    async fn channel(&self, _id: &Id) -> Result<&Channel> {
        unimplemented!()
    }

    async fn contact(&self, _id: &Id) -> Result<&Contact> {
        unimplemented!()
    }

    async fn contacts(&self) -> Result<Vec<&Contact>> {
        unimplemented!()
    }

    async fn add_contact(&mut self, _id: &Id, _home_peer_id: Option<&Id>, _session_key: &[u8], _remark: Option<&str>) -> Result<()> {
        unimplemented!()
    }

    async fn update_contact(&mut self, _contact: Contact) -> Result<()> {
        unimplemented!()
    }

    async fn remove_contact(&mut self, _id: &Id) -> Result<()> {
        unimplemented!()
    }

    async fn remove_contacts(&mut self, _ids: Vec<&Id>) -> Result<()> {
        unimplemented!()
    }
}
