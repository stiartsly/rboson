use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;
use serde::{Serialize, de::DeserializeOwned};
use log::{error, warn, info, debug, trace};
use tokio::task::JoinHandle;
use serde_cbor;
use sha2::{Digest, Sha256};
use md5;
use url::Url;
use rumqttc::{
    MqttOptions,
    AsyncClient,
    SubscribeFilter,
    Event,
    Packet,
    Outgoing //, Incoming
};

use crate::{
    unwrap,
    lock,
    as_secs,
    Id,
    Identity,
    PeerInfo,
    cryptobox::Nonce,
    signature,
    core::{
        Error,
        Result,
        CryptoIdentity,
        CryptoContext
    }
};

use crate::messaging::{
    ClientDevice,
    ServiceIds,
    UserAgentCaps,
    UserAgent,
    InviteTicket,
    Contact,
    ClientBuilder,
    MessagingAgent,
    ConnectionListener,
    ChannelListener,
    ContactListener,
    ProfileListener,
    profile::Profile,
    api_client::{self, APIClient},
    channel::{self, Role, Permission, Channel},
    message_listener::{
        MessageListenerMut,
    },
    rpc::{
        method::RPCMethod,
        request::RPCRequest,
        response::RPCResponse,
        notif::{events, Notification, ChannelMembersRoleUpdated},
        params::{self, Parameters},
        promise::{self, Ack, Promise, Waiter},
    },
    message::{
        MessageType,
        Message as Msg,
        Builder as MsgBuilder
    },
    internal::contacts_update::ContactsUpdate,
};

#[macro_export]
macro_rules! ua {
    ($me:expr) => {{
        $me.ua.lock().unwrap()
    }};
}

#[allow(dead_code)]
pub struct MessagingClient {
    peer            : PeerInfo,
    user            : CryptoIdentity,
    device          : CryptoIdentity,
    client_id       : String,

    inbox           : String,
    outbox          : String,
    broadcast       : String,

    base_index      : RefCell<u32>,

    service_info    : Option<api_client::MessagingServiceInfo>,

    server_context  : Option<Arc<Mutex<CryptoContext>>>,
    self_context    : Option<Arc<Mutex<CryptoContext>>>,

    api_client      : Option<APIClient>,
    disconnect      : bool,
    connected       : Arc<Mutex<bool>>,

    worker_task     : Option<JoinHandle<()>>,
    worker_client   : Option<Arc<Mutex<AsyncClient>>>,

    pending_calls   : Arc<Mutex<HashMap<u32, RPCRequest>>>,

    ua              : Arc<Mutex<UserAgent>>,
}

#[allow(dead_code)]
impl MessagingClient {
    pub(crate) fn new(b: ClientBuilder) -> Result<Self> {
        let ua = b.ua();
        if !lock!(ua).is_configured() {
            return Err(Error::State("User agent is not configured".into()));
        }

        let peer = lock!(ua).peer().clone();
        let user = lock!(ua).user().unwrap().identity().clone();
        let device = lock!(ua).device().unwrap().identity().unwrap().clone();

        println!("peerid    : {}", peer.id());
        println!("userid    : {}", user.id());
        println!("deviceid  : {}", device.id());

        lock!(ua).harden();
        drop(ua);

        let userid = user.id().to_base58();
        let clientid = bs58::encode({
            md5::compute(device.id().as_bytes()).0
        }).into_string();

        Ok(Self {
            peer,
            user,
            device,
            service_info    : None,

            client_id       : clientid,
            inbox           : format!("inbox/{userid}",),
            outbox          : format!("outbox/{userid}",),
            broadcast       : format!("broadcast"),

            base_index      : RefCell::new(0),

            api_client      : None,
            disconnect      : false,
            connected       : Arc::new(Mutex::new(false)),

            worker_client   : None,
            worker_task     : None,

            self_context    : None,
            server_context  : None,

            pending_calls   : Arc::new(Mutex::new(HashMap::new())),

            ua              : b.ua().clone(),
        })
    }

    fn worker(&self) -> Arc<Mutex<AsyncClient>> {
        self.worker_client
            .as_ref()
            .expect("MQTT client should be created")
            .clone()
    }

    fn api_client(&mut self) -> &mut APIClient {
        self.api_client
            .as_mut()
            .expect("API client should be created")
    }

    fn self_ctxt(&self) -> &Arc<Mutex<CryptoContext>> {
        self.self_context
            .as_ref()
            .expect("Self crypto context should be created")
    }

    fn server_ctxt(&self) -> &Arc<Mutex<CryptoContext>> {
        self.server_context
            .as_ref()
            .expect("Server crypto context should be created")
    }

    fn pending_calls(&self) -> Arc<Mutex<HashMap<u32, RPCRequest>>> {
        self.pending_calls.clone()
    }

    pub(crate) fn next_index(&self) -> u32 {
        self.base_index.replace_with(|&mut v| v + 1);
        *self.base_index.borrow()
    }

    pub fn load_access_token(&mut self) -> Result<Option<String>> {
        // TODO:
        Ok(Some("TODO".into()))
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Messaging client Started!");

        _ = self.load_access_token()?;

        let peer = ua!(self).peer().clone();
        let user = ua!(self).user().unwrap().identity().clone();
        let device = ua!(self).device().unwrap().identity().unwrap().clone();
        let api_url = match peer.alternative_url().as_ref() {
            None => Err(Error::State("Alternative URL should be set".into())),
            Some(url) => Url::parse(url).map_err(|e|
                Error::State(format!("Failed to parse API URL: {e}"))
            )
        }?;

        let api_client = api_client::Builder::new()
            .with_base_url(&api_url)
            .with_home_peerid(peer.id())
            .with_user_identity(&user)
            .with_device_identity(&device)
            //.with_access_token(self.load_access_token().ok())
            .with_access_token_refresh_handler(|_| {})
            .build()?;

        self.api_client = Some(api_client);
        let version = match lock!(self.ua).contacts_version() {
            Ok(v) => Some(v),
            Err(e) => {
                warn!("Failed to retrieve contact version from local agent with error {e}");
                None
            }
        };

        if version.is_none() {
            let mut version = self.api_client().fetch_contacts_update(
                version.as_deref()
            ).await?;

            if let Some(version_id) = version.version_id() {
                _ = ua!(self).put_contacts_update(
                    &version_id,
                    version.contacts().as_slice()
                ).map_err(|e|{
                    warn!("Failed to put contacts update to local agent: {e}, ignored this failure.");
                });
            }
        }

        self.service_info = Some(self.api_client().service_info().await?);
        self.self_context = Some(Arc::new(Mutex::new(self.user.create_crypto_context(user.id())?)));
        self.server_context = Some(Arc::new(Mutex::new(self.user.create_crypto_context(peer.id())?)));
        Ok(())
    }

    pub async fn stop(&mut self, forced: bool) {
        _ = self.disconnect().await;
        self.disconnect = true;

        if let Some(ctxt) = self.self_context.take() {
            drop(ctxt);
        }
        if let Some(ctxt) = self.server_context.take() {
            drop(ctxt);
        }

        if let Some(task) = self.worker_task.take() {
            if forced {
                task.abort()
            };
            _ = task.await;
        };

        info!("Messaging client stopped ...");
        self.worker_task = None;
        self.worker_client = None;
    }

    fn password(user: &CryptoIdentity, device: &CryptoIdentity) -> String {
        let nonce = Nonce::random();
        let usr_sig = user.sign_into(nonce.as_bytes()).unwrap();
        let dev_sig = device.sign_into(nonce.as_bytes()).unwrap();

        let mut password = Vec::<u8>::with_capacity(
            nonce.size() + usr_sig.len() + dev_sig.len()
        );

        password.extend_from_slice(nonce.as_bytes());
        password.extend_from_slice(usr_sig.as_slice());
        password.extend_from_slice(dev_sig.as_slice());

        bs58::encode(password).into_string()
    }

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }

    async fn sign_into_invite_ticket(&self,
        channel_id: &Id,
        invitee: Option<&Id>
    ) -> Result<InviteTicket> {
        let Some(channel) = ua!(self).channel(channel_id)? else {
            return Err(Error::Argument("No channel {} was found in local agent.".into()));
        };

        let expire = SystemTime::now() + Duration::from_secs(InviteTicket::EXPIRATION);
        let expire = as_secs!(expire);
        let sha256 = {
            let mut sha256 = Sha256::new();
            sha256.update(channel_id.as_bytes());
            sha256.update(self.user.id().as_bytes());

            let invitee = match invitee {
                Some(id) => id,
                None => &Id::max()
            };
            sha256.update(invitee.as_bytes());
            sha256.update(&expire.to_le_bytes());
            sha256.finalize().to_vec()
        };

        let sig = self.user.sign_into(&sha256)?;
        let sk = channel.session_keypair().unwrap().private_key();
        let sk = match invitee {
            Some(invitee) => self.user.encrypt_into(invitee, sk.as_bytes())?,
            None => sk.as_bytes().to_vec()
        };

        Ok(InviteTicket::new(
            channel_id.clone(),
            self.user.id().clone(),
            invitee.is_none(),
            expire,
            sig,
            Some(sk)
        ))
    }

    async fn send_rpc_request(&self,
        recipient: &Id,
        req: RPCRequest
    ) -> Result<()> {
        let msg = MsgBuilder::new(self, MessageType::Call)
            .with_to(recipient)
            .with_body(serde_cbor::to_vec(&req).unwrap())
            .build();

        self.pending_calls.lock().unwrap().insert(req.id(), req);
        self.send_msg(msg).await
    }

    async fn send_msg(&self, msg: Msg) -> Result<()> {
        let encryption_needed = |v: &Msg| {
            let with_body = match v.body() {
                Some(b) => !b.is_empty(),
                None => false
            };
            with_body && v.to() != self.peer.id()
        };

        let encrypt_cb_for_msg = |msg: &Msg| -> Result<Vec<u8>> {
            let recipient = ua!(self).contact(msg.to())?;
            let Some(rec) = recipient else {
                let estr = format!("Failed to send message to unknown recipient {}", msg.to());
                error!("{}", estr);
                // TODO error.
                return Err(Error::State(estr));
            };

            let Some(sid) = rec.session_id() else {
                let estr = format!("INTERNAL error: recipient {} has no session key.", msg.to());
                error!("{}", estr);
                return Err(Error::State(estr));
            };

            self.user.create_crypto_context(&sid)?
                .encrypt_into(unwrap!(msg.body()))
        };

        let encrypt_cb_for_call = |msg: &Msg| -> Result<Vec<u8>> {
            self.user.create_crypto_context(msg.to())?
                .encrypt_into(unwrap!(msg.body()))
        };

        let msg = if encryption_needed(&msg) {
            let msg_type = msg.message_type();
            let encrypted = match msg_type {
                MessageType::Message    => encrypt_cb_for_msg(&msg)?,
                MessageType::Call       => encrypt_cb_for_call(&msg)?,
                _ => {
                    panic!("INTERNAL fatal: unsupported msg type {:?}", msg.message_type());
                }
            };
            msg.dup_from(encrypted)
        } else {
            msg
        };

        self.publish_msg(&msg).await
    }

    async fn publish_msg(&self, msg: &Msg) -> Result<()> {
        let outbox = self.outbox.as_str();
        let payload = serde_cbor::to_vec(msg).unwrap();
        let payload = self.server_ctxt().lock().unwrap().encrypt_into(&payload)?;

        self.worker().lock().unwrap().publish(
            outbox,
            rumqttc::QoS::AtLeastOnce,
            false,
            payload
        ).await.map_err(|e| {
            Error::State(format!("Internal error {e}: failed to publish message: {}", e))
        })?;

        debug!("Message published to outbox {}", outbox);

        // TODO: pending messages.
        // TODO: sending messages;
        Ok(())
    }

    async fn attempt_connect(&mut self, url: &Url) -> Result<()> {
        if self.disconnect {
            return Err(Error::State("Client is stopped".into()));
        }

        let options = {
            let mut options = MqttOptions::new(
                &self.client_id,
                url.host().unwrap().to_string(),
                url.port().unwrap_or(1883) as u16
            );
            options.set_credentials(
                self.user.id().to_base58(),
                Self::password(&self.user, &self.device)
            );
            options.set_max_packet_size(16*1024, 18*1024);
            options.set_keep_alive(Duration::from_secs(60));
            options.set_clean_session(false);
            options
        };

        if url.scheme() == "ssl" {
            //mqtt_options.set_transport(true);
            // TODO:
        }

        let (client, mut eventloop) = AsyncClient::new(options, 10);
        let client = Arc::new(Mutex::new(client));
        let mut worker = MessagingWorker::new(self, client.clone());

        self.worker_client = Some(client);
        self.worker_task   = Some(tokio::spawn(async move {
            loop {
                let event = match eventloop.poll().await {
                    Ok(event) => event,
                    Err(e) => {
                        error!("MQTT event loop error: {}, break the loop.", e);
                        break;
                    }
                };

                match event {
                    Event::Incoming(packet) => worker.on_incoming_msg(packet).await,
                    Event::Outgoing(packet) => worker.on_outgoing_msg(packet),
                }
            }
        }));

        Ok(())
    }

    async fn do_connect(&mut self) -> Result<()> {
        if let Some(_) = self.worker_client.as_ref() {
            if self.is_connected() {
                info!("Already connected to the messaging server");
                return Ok(());
            }
        }

        info!("Connecting to the messaging server ...");
        self.disconnect = false;
        ua!(self).on_connecting();

        let urls = vec![
            Url::parse("tcp://155.138.245.211:1883").unwrap(),  // TODO:
        ];
        self.attempt_connect(&urls[0]).await?;

        debug!("Subscribing to the messages ....");
        let topics = vec![
            SubscribeFilter::new(self.inbox.clone(), rumqttc::QoS::AtLeastOnce),
            SubscribeFilter::new(self.outbox.clone(), rumqttc::QoS::AtLeastOnce),
            SubscribeFilter::new(self.broadcast.clone(), rumqttc::QoS::AtLeastOnce),
        ];
        self.worker().lock().unwrap().subscribe_many(topics).await.map(|_| {
            info!("Subscribed to the messages successfully");
        }).map_err(|e| {
            let errstr = format!("Failed to connect to the messaging server: {}", e);
            error!("{}", errstr);
            ua!(self).on_disconnected();
            Error::State(errstr)
        })
    }

    async fn push_contacts_update(&mut self,
        updated_contacts: Vec<Contact>
    ) -> Result<String> {

        let current_version = lock!(self.ua).contacts_version()?;

        let arc = Arc::new(Mutex::new(promise::StringVal::new()));
        let fut = Promise::PushContactsUpdate(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ContactPush,
            Some(Parameters::ContactsUpdate(
                ContactsUpdate::new(Some(current_version), updated_contacts)
            ))
        );
        self.send_rpc_request(
            &self.peer.id(),
            req
        ).await?;

        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    fn is_started(&self) -> bool {
        self.api_client.is_some()
    }
}

unsafe impl Send for MessagingClient {}
unsafe impl Sync for MessagingClient {}

impl MessagingAgent for MessagingClient {
    fn userid(&self) -> &Id {
        self.user.id()
    }

    fn user_agent(&self) -> Arc<Mutex<UserAgent>> {
        self.ua.clone()
    }

    async fn close(&mut self) -> Result<()> {
        // TODO:
        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        self.do_connect().await
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnected !!!");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *lock!(self.connected)
    }

    /*
    fn message(&mut self) -> MessageBuilder {
        MessageBuilder::new(self, MessageType::Message)
    }
    */

    async fn update_profile(&mut self,
        name: Option<&str>,
        avatar: bool
    ) -> Result<()> {
        let Some(client) = self.api_client.as_mut() else {
            return Err(Error::State("The client is not started yet".into()));
        };
        client.update_profile(
            name.map(|n| n.nfc().collect::<String>()).as_deref(),
            avatar
        ).await
    }

    async fn upload_avatar(&mut self,
        content_type: &str,
        avatar: &[u8]
    ) -> Result<String> {
        let Some(client) = self.api_client.as_mut() else {
            return Err(Error::State("The client is not started yet".into()));
        };
        client.upload_avatar(
            content_type,
            avatar
        ).await
    }

    async fn upload_avatar_from_file(
        &mut self,
        content_type: &str,
        file_name: &str
    ) -> Result<String> {
        let Some(client) = self.api_client.as_mut() else {
            return Err(Error::State("The client is not started yet".into()));
        };
        client.upload_avatar_from_file(
            content_type,
            file_name.into()
        ).await
    }

    async fn devices(&mut self) -> Result<Vec<ClientDevice>> {
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::DevicesVal::new()));
        let fut = Promise::GetDeviceList(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::DeviceList,
            None,
        ).with_promise(fut.clone());

        self.send_rpc_request(
            &self.peer.id(),
            req
        ).await?;

        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn revoke_device(
        &mut self,
        device_id: &Id
    ) -> Result<()> {
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::RevokeDevice(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::DeviceRevoke,
            Some(Parameters::RevokeDevice(device_id.clone()))
        ).with_promise(fut.clone());

        self.send_rpc_request(
            &self.peer.id().clone(),
            req
        ).await?;

        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn create_channel(&self,
        permission: Option<channel::Permission>,
        name: &str,
        notice: Option<&str>
    ) -> Result<Channel> {
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let keypair = signature::KeyPair::random();
        let session_id = Id::from(keypair.public_key());
        let params = params::ChannelCreate::new(
            session_id,
            permission.unwrap_or(channel::Permission::OwnerInvite),
            Some(name.into()),
            notice.map(|n| n.into()),
        );

        let arc = Arc::new(Mutex::new(promise::ChannelVal::new()));
        let fut = Promise::CreateChannel(arc.clone());
        let cookie = lock!(self.self_ctxt()).encrypt_into(
            keypair.private_key().as_bytes()
        )?;
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelCreate,
            Some(Parameters::CreateChannel(params))
        )
        .with_promise(fut.clone())
        .with_cookie(cookie);

        self.send_rpc_request(
            unwrap!(self.service_info).peerid(),  // why not peerid.
            req
        ).await?;

        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn remove_channel(&self,
        channel_id: &Id
    ) -> Result<()> {
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::RemoveChannel(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelDelete,
            None
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn join_channel(&mut self,
        ticket: &InviteTicket
    ) -> Result<Channel> {
        if ticket.session_key().is_none() {
            Err(Error::Argument("Invite ticket does not contain session key".into()))?
        }
        if ticket.is_expired() {
            Err(Error::Argument("Invite ticket is expired".into()))?
        }
        if !ticket.is_valid(self.user.id()) {
            Err(Error::Argument("Invite ticket is not valid for this user".into()))?
        }

        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        // check session key
        let session_key = match ticket.is_public() {
            true => ticket.session_key().unwrap().to_vec(),
            false => {
                self.user.decrypt_into(
                    ticket.inviter(),
                    ticket.session_key().unwrap()
                )?
            }
        };
        _ = signature::KeyPair::try_from(
            session_key.as_slice()
        ).map_err(|_| {
            Error::Argument(format!("Invalid member private key"))
        })?;

        let arc = Arc::new(Mutex::new(promise::ChannelVal::new()));
        let fut = Promise::JoinChannel(arc.clone());
        let cookie = lock!(self.self_ctxt()).encrypt_into(
            session_key.as_slice()
        )?;
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelJoin,
            Some(Parameters::JoinChannel(ticket.proof().clone()))
        )
        .with_promise(fut.clone())
        .with_cookie(cookie);

        self.send_rpc_request(ticket.channel_id(), req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn leave_channel(&mut self, channel_id: &Id) -> Result<()> {
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::LeaveChannel(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelLeave,
            None
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn create_invite_ticket(&mut self,
        channel_id: &Id,
        invitee: Option<&Id>
    ) -> Result<InviteTicket> {
        self.sign_into_invite_ticket(channel_id, invitee).await
    }

    async fn set_channel_owner(&mut self,
        channel_id: &Id,
        new_owner: &Id
    ) -> Result<()> {
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {channel_id} found from user agent".into()))?
        };
        if !channel.is_owner(self.user.id()) {
            Err(Error::Argument("Not channel owner".into()))?
        }
        if channel.is_member(new_owner) {
            Err(Error::Argument("New owner is not in the channel".into()))?
        }

        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::SetChannelOwner(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelOwner,
            Some(Parameters::SetChannelOwner(new_owner.clone()))
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_permission(&mut self,
        channel_id: &Id,
        permission: Permission
    ) -> Result<()> {
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };
        if !channel.is_owner(self.user.id()) {
            Err(Error::Argument("Not channel owner".into()))?
        }

        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::SetChannelPerm(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelPermission,
            Some(Parameters::SetChannelPermission(permission))
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_name(&mut self,
        channel_id: &Id,
        name: Option<&str>
    ) -> Result<()> {
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::Argument("Not channel owner or moderator".into()))?
        }
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let name = name.map(|n|
            match !n.is_empty() {
                true => Some(n.nfc().collect::<String>()),
                false => None,
            }
        ).flatten();

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::SetChannelName(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelName,
            name.filter(|v| !v.is_empty()).map(|v| {
                let nfc = v.nfc().collect::<String>();
                Parameters::SetChannelNotice(nfc)
            })
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_notice(&mut self,
        channel_id: &Id,
        notice: Option<&str>
    ) -> Result<()> {
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::Argument("Not channel owner or moderator".into()))?
        }
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::SetChannelNotice(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelNotice,
            notice.filter(|v| !v.is_empty()).map(|v| {
                let nfc = v.nfc().collect::<String>();
                Parameters::SetChannelNotice(nfc)
            })
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_member_role(&mut self,
        channel_id: &Id,
        members: Vec<&Id>,
        role: Role
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::Argument("Not channel owner or moderator".into()))?
        }
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let role = params::ChannelMemberRole::new(
            members.into_iter().map(|id| id.clone())
                .collect::<Vec<Id>>(),
            role,
        );
        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::SetChannelMemberRole(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelRole,
            Some(Parameters::SetChannelMemberRole(role))
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn ban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<&Id>,
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(())
        }
        let Some(ch) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };
        let userid = self.user.id();
        if !ch.is_owner(userid) && !ch.is_moderator(userid) {
            Err(Error::Argument("Not channel owner or moderator".into()))?
        }
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let members = members.into_iter()
            .map(|id| id.clone())
            .collect::<Vec<Id>>();

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::BanChannelMembers(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelBan,
            Some(Parameters::BanChannelMembers(
                members.into_iter().map(|v| v.clone()).collect::<Vec<Id>>()
            ))
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn unban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<&Id>
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };

        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let fut = Promise::UnbanChannelMembers(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelUnban,
            Some(Parameters::UnbanChannelMembers(
                members.into_iter().map(|id| id.clone()).collect::<Vec<Id>>()
            ))
        ).with_promise(fut.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(fut).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn remove_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<&Id>
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }
        let Some(channel) = ua!(self).channel(channel_id)? else {
            Err(Error::Argument("No channel {{{channel_id}}} was found".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        if !self.is_connected() {
            return Err(Error::State("The client is not connected yet".into()));
        }

        let arc = Arc::new(Mutex::new(promise::BoolVal::new()));
        let promise = Promise::RemoveChannelMembers(arc.clone());
        let req = RPCRequest::new(
            self.next_index(),
            RPCMethod::ChannelRemove,
            Some(Parameters::RemoveChannelMembers(
                members.into_iter().map(|id| id.clone()).collect::<Vec<Id>>())
            )
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;
        match Waiter::new(promise).await {
            Ok(_) => lock!(arc).result(),
            Err(e) => Err(e)
        }
    }

    async fn add_contact(&mut self,
        id: &Id,
        home_peer_id: Option<&Id>,
        session_key: &[u8],
        remark: Option<&str>
    ) -> Result<Contact> {
        _ = signature::KeyPair::try_from(
            session_key
        ).map_err(|_| {
            Error::Argument(format!("Invalid session private key"))
        })?;

        let contact = Contact::new1(
            id.clone(),
            home_peer_id.cloned(),
            session_key.to_vec(),
            remark.map(|r| r.nfc().collect::<String>())
        )?;

        self.push_contacts_update(vec![contact.clone()]).await
            .map(|_| contact)
    }

    async fn contact(&self, id: &Id) -> Result<Option<Contact>> {
        let ua = self.ua.clone();
        let id = id.clone();

        tokio::spawn(async move {
            lock!(ua).contact(&id)
        }).await.unwrap()
    }

    async fn channel(&self, id: &Id) -> Result<Option<Channel>> {
        let ua = self.ua.clone();
        let id = id.clone();

        tokio::spawn(async move {
            lock!(ua).channel(&id)
        }).await.unwrap()
    }

    async fn contacts(&self) -> Result<Vec<Contact>> {
        let ua = self.ua.clone();

        tokio::spawn(async move {
            lock!(ua).contacts()
        }).await.unwrap()
    }

    async fn update_contact(&mut self,
        contact: Contact
    ) -> Result<Contact> {
        if !contact.is_modified() {
            Err(Error::Argument("Contact is not modified".into()))?;
        }

        self.push_contacts_update(vec![contact.clone()]).await
            .map(|_| contact)
    }

    async fn remove_contact(&mut self, id: &Id) -> Result<()> {
        self.remove_contacts(vec![id]).await
    }

    async fn remove_contacts(&mut self, _ids: Vec<&Id>) -> Result<()> {
        unimplemented!()
    }
}

struct MessagingWorker {
    ua              : Arc<Mutex<UserAgent>>,
    _worker_client   : Arc<Mutex<AsyncClient>>,

    self_context    : Arc<Mutex<CryptoContext>>,
    server_context  : Arc<Mutex<CryptoContext>>,
    disconnect      : bool,
    failures        : u32,
    connected       : Arc<Mutex<bool>>,


    peer            : PeerInfo,

    inbox           : String,
    outbox          : String,
    broadcast       : String,

    pending_calls   : Arc<Mutex<HashMap<u32, RPCRequest>>>,

    user            : CryptoIdentity
}

impl MessagingWorker {
    fn new(client: &MessagingClient,  mqttc: Arc<Mutex<AsyncClient>>) -> Self {
        Self {
            ua              : client.ua.clone(),
            _worker_client   : mqttc,

            self_context    : client.self_ctxt().clone(),
            server_context  : client.server_ctxt().clone(),
            disconnect      : false,
            failures        : 0,
            peer            : client.peer.clone(),
            connected       : client.connected.clone(),

            inbox           : client.inbox.clone(),
            outbox          : client.outbox.clone(),
            broadcast       : client.broadcast.clone(),

            pending_calls   : client.pending_calls(),
            user            : client.user.clone(),
        }
    }

    fn is_me(&self, id: &Id) -> bool {
        self.user.id() == id
    }

    async fn on_incoming_msg(&mut self, packet: Packet) {
        match packet {
            Packet::Publish(v)  => self.on_publish(v).await,
            Packet::PubAck(_)   => {},
            Packet::SubAck(_)   => {},
            Packet::UnsubAck(_) => {},
            Packet::Disconnect  => self.on_disconnect(),
            Packet::PingResp    => self.on_ping_rsp(),
            Packet::ConnAck(_)  => self.on_connected(),
            _ => {
                error!("Fatail error: unknown MQTT event: {:?}", packet);
                panic!();
            }
        }
    }

    fn on_outgoing_msg(&mut self, _packet: Outgoing) {
        match _packet {
            Outgoing::Publish(_pktid) => {},
            _ => {},
        }
    }

    fn on_disconnect(&mut self) {
        if self.disconnect {
            ua!(self).on_disconnected();
            info!("disconnected!");
            return;
        }

        self.failures += 1;
        *lock!(self.connected) = false;
        error!("Connection lost, attempt to reconnect in {} seconds...", 5); // TODO:

    }

    fn on_ping_rsp(&mut self) {
        trace!("Ping response received");
    }

    fn on_connected(&mut self) {
        self.failures = 0;
        *lock!(self.connected) = true;
        info!("Connected to the messaging server");
        ua!(self).on_connected();
    }

    async fn on_publish(&mut self, data: rumqttc::Publish) {
        let topic = data.topic.as_str();
        debug!("Got message on topic: {}", topic);

        let decrypted = match lock!(self.server_context).decrypt_into(&data.payload) {
            Ok(v) => v,
            Err(e) => {
                error!("Error decrypting message payload from {topic}: {e}");
                return;
            }
        };

        let mut msg = match serde_cbor::from_slice::<Msg>(&decrypted) {
            Ok(v) => v,
            Err(e) => {
                error!("Error deserializing message from {topic}: {e}");
                return;
            }
        };

        if !msg.is_valid() {
            error!("Received invalid message from {topic}, ignored");
            return;
        }
        msg.mark_encrypted(true);

        if topic == self.inbox {
            self.on_inbox_msg(msg).await;
        } else if topic == self.outbox {
            self.on_outbox_msg(msg).await;
        } else if topic == self.broadcast {
            self.on_broadcast_msg(msg);
        } else {
            error!("Received message with unknown topic: {topic}, ignored");
            return;
        }
    }

    async fn on_inbox_msg(&mut self, mut msg: Msg) {
        let need_decryption = |v: &Msg| {
            let with_body = match v.body() {
                Some(b) => !b.is_empty(),
                None => false
            };
            with_body && v.from() != self.peer.id()
        };

        if !need_decryption(&msg) {
            msg.mark_encrypted(false);
            return self.process_msg(msg).await;
        }

        if self.is_me(msg.to()){
            match msg.message_type() {
                MessageType::Message => {
                    // Message: sender -> me
                    // The body is encrypted using the sender's private key
                    // and the session public key associated with that sender.
                    let sender = ua!(self).contact(msg.from());
                    let Ok(Some(sender)) = sender else {
                        warn!("Sender {} not in contact list, ignored", msg.from());
                        return;
                    };
                    if sender.session_keypair().is_none() {
                        warn!("No session key attached to sender {}, ignored", msg.from());
                        return;
                    }

                    if let Err(e) = msg.decrypt_body(unwrap!(sender.rx_crypto_context())) {
                        warn!("Error decrypting message body: {}, ignored", e);
                        return;
                    };
                },
                MessageType::Call => {
                    // Call: sender(user | channel) -> me
                    // The body is encrypted using the sender's private key
                    // and my public key.

					// TODO: CHECKME - cache the CryptoContext?
                    let ctxt = self.user.create_crypto_context(msg.from());
                    if let Err(e) = msg.decrypt_body(unwrap!(ctxt)) {
                        warn!("Error decrypting call body: {}, ignored", e);
                        return;
                    };
                },
                _ => {
                    // Notification: !homePeer -> me
					// The body is encrypted using the sender's private key
					// and my public key.

					// TODO: CHECKME - cache the CryptoContext?
                    let ctxt = self.user.create_crypto_context(msg.from());
                    if let Err(e) = msg.decrypt_body(unwrap!(ctxt)) {
                        warn!("Error decrypting notitification body: {}, ignored", e);
                        return;
                    };
                }
            }
        } else {
            let Ok(Some(channel)) = ua!(self).channel(msg.to()) else {
                warn!("No channel {{{}}} found, ignored", msg.to());
                return;
            };
            if channel.session_keypair().is_none() {
                warn!("No session key for channel {{{}}}, ignored", msg.to());
                return;
            };

            match msg.message_type() {
                MessageType::Message => {
                    // Message: sender -> channel
                    // The body is encrypted using the sender's private key
                    // and the session public key of channel.
                    let Some(ctxt) = channel.rx_crypto_context_by(msg.from()) else {
                        warn!("No crypto context found for sender {{{}}} in channel {{{}}}, ignored",
                            msg.from(), msg.to());
                        return;
                    };
                    if let Err(e) = msg.decrypt_body(ctxt) {
                        warn!("Error decrypting message body: {}, ignored", e);
                        return;
                    }
                },
                MessageType::Notification => {
                    // Message: channel -> channel
                    // The body is encrypted using the channel's private key
                    // and the channel session's public key

                    let Ok(ctxt) = channel.rx_crypto_context() else {
                        warn!("No crypto context found for channel {{{}}}, ignored", msg.to());
                        return;
                    };
                    if let Err(e) = msg.decrypt_body(ctxt) {
                        warn!("Error decrypting notification body: {}, ignored", e);
                        return;
                    }
                },
                MessageType::Call => {
                    panic!("Should no Call message type sent to channel")
                }
            }
        }

        if msg.is_encrypted() {
            warn!("Message from unknow sender {}, keep in encrypted", msg.from());
        }
        self.process_msg(msg).await
    }

    async fn on_outbox_msg(&mut self, mut msg: Msg) {
        let need_decryption = |v: &Msg| {
            let with_body = match v.body() {
                Some(b) => !b.is_empty(),
                None => false
            };
            with_body && v.from() != self.peer.id()
        };

        if true { // TODO:
            warn!("Outgoing message received on outbox, ignored");
            return;
        } else if need_decryption(&msg) {
            match msg.message_type() {
                MessageType::Message => {
                    // Message: me -> recipient
                    // The body is encrypted using my private key
                    // and the session public key associated with the recipient.
                    let recipient = ua!(self).contact(msg.to());
                    let Ok(Some(recipient)) = recipient else {
                        warn!("Recipient {} not in contact list, ignored", msg.to());
                        return;
                    };
                    if recipient.session_keypair().is_none() {
                        warn!("No session key attached to recipient {}, ignored", msg.to());
                        return;
                    }
                },
                MessageType::Call => {
                    // Call: me -> recipient (user | channel)
                    // The body is encrypted using my private key
                    // and the recipient's public key.
                },
                _ => {}
            }
        } else {
            // service RPC requests: me -> servicePeer
            // body is encrypted together with the message envelope.
            // Or: empty body
            msg.mark_encrypted(false);
        }

        match msg.message_type() {
            MessageType::Message => self.on_sent(msg).await,
            MessageType::Call => self.on_rpc_request(msg).await,
            _ => warn!("Unexpected message type on outbox, ignored")
        }
    }

    async fn on_sent(&mut self, _msg: Msg) {
        unimplemented!()
    }

    fn on_broadcast_msg(&mut self, mut msg: Msg) {
        // Broadcast notifications from the service peer.
		// Message body is encrypted with the message envelope,
		// it was already decrypted here

        msg.mark_encrypted(false);
        ua!(self).on_broadcast(msg);
    }

    async fn process_msg(&mut self, msg: Msg) {
        match msg.message_type() {
            MessageType::Message => {
                self.on_msg(msg).await;
            },
            MessageType::Call => {
                self.on_rpc_response(msg).await;
            },
            MessageType::Notification => {
                self.on_notification(msg).await;
            }
        }
    }

    async fn on_msg(&mut self, msg: Msg) {
        let need_refresh = |contact: Option<&Contact>| {
            match contact {
                Some(c) => c.is_staled(),
                None => true
            }
        };
        let conversation_id = match !self.is_me(msg.to()) {
            true => msg.to().clone(),
            false => msg.from().clone()
        };

        lock!(self.ua).on_message(msg);

        let ua = self.ua.clone();
        _ = tokio::spawn(async move {
            let Ok(contact) = lock!(ua).contact(&conversation_id) else {
                error!("Error retrieving contact {} from user agent", conversation_id);
                return;
            };
            // check if the contact need to be update
            if need_refresh(contact.as_ref()) {
                //self.arefresh_profile(&conv_id).await.on_success(|profile| {
                //    self.ua().lock().unwrap().on_contact_profile(&conv_id, profile);
                //});
            }
        }).await;
    }

    async fn on_rpc_response(&mut self, msg: Msg) {
        let Some(body) = msg.body().filter(|b| !b.is_empty()) else {
            warn!("Empty RPC response received from {}, ignored", msg.from());
            return;
        };

        {
            use hex::ToHex;
            let body_hex = body.encode_hex::<String>();
            println!("RPC response body (hex): {}", body_hex);
        }

        let Ok(mut preparsed) = RPCResponse::from(body) else {
            error!("Failed to parse RPC response from {}, message ignored", msg.from());
            return;
        };

        let Some(call) = self.pending_calls.lock().unwrap().remove(preparsed.id()) else {
            error!("Unmatched RPC response ID {} from {}, message ignored", preparsed.id(), msg.from());
            return;
        };

        match call.method() {
            RPCMethod::DeviceList => {
                let complete = |rc: Result<Vec<ClientDevice>>| {
                    if let Some(Promise::GetDeviceList(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                let devices = match preparsed.result::<Vec<ClientDevice>>() {
                    Ok(v) => v,
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                complete(Ok(devices))
            },

            RPCMethod::DeviceRevoke => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::RevokeDevice(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                complete(Ok(()))
            },

            RPCMethod::ContactPush => {
                // TODO:
            },

            RPCMethod::ContactClear => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::ContactClear(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                lock!(self.ua).on_contacts_cleared();
                complete(Ok(()))
            },

            RPCMethod::ChannelCreate => {
                let complete = |rc: Result<Channel>| {
                    if let Some(Promise::CreateChannel(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                let mut channel = match preparsed.result::<Channel>() {
                    Ok(v) => v,
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                let session_key = match lock!(self.self_context).decrypt_into(
                    unwrap!(call.cookie())) {
                    Ok(v) => v,
                    Err(e) => {
                        complete(err_from(e));
                        return
                    }
                };
                if let Err(e) = channel.set_session_key(&session_key) {
                    complete(err_from(e));
                    return
                }

                //let channel_id = channel.id().clone();
                lock!(self.ua).on_joined_channel(&channel);
                complete(Ok(channel));

               // if call.is_initiator() {
               //     _ = self.channel_members(&channel_id).await;
               // }
            }
            RPCMethod::ChannelDelete => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::RemoveChannel(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => {
                        let estr = format!("Internal error: no channel {} found", msg.from());
                        complete(Err(Error::State(estr)));
                        return;
                    },
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                ua!(self).on_channel_deleted(&channel);
                complete(Ok(()))
            },
            RPCMethod::ChannelJoin => {
                let complete = |rc: Result<Channel>| {
                    if let Some(Promise::JoinChannel(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                let mut channel = match preparsed.result::<Channel>() {
                    Ok(v) => v,
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                let session_key = match lock!(self.self_context).decrypt_into(
                    unwrap!(call.cookie())) {
                    Ok(v) => v,
                    Err(e) => {
                        complete(err_from(e));
                        return
                    }
                };
                if let Err(e) = channel.set_session_key(&session_key) {
                    complete(err_from(e));
                    return
                }

                //let channel_id = channel.id().clone();
                lock!(self.ua).on_joined_channel(&channel);
                complete(Ok(channel));

               // if call.is_initiator() {
               //     _ = self.channel_members(&channel_id).await;
               // }
            },
            RPCMethod::ChannelLeave => { // TODO:
                let complete = |rc: Result<()>| {
                    if let Some(Promise::LeaveChannel(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => {
                        let estr = format!("Internal error: no channel {} found", msg.from());
                        complete(Err(Error::State(estr)));
                        return;
                    },
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                ua!(self).on_left_channel(&channel);
                complete(Ok(()))
            },
            RPCMethod::ChannelOwner => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::SetChannelOwner(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let mut channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::SetChannelOwner(new_owner) = unwrap!(call.params()) {
                    channel.set_owner(new_owner.clone());
                    ua!(self).on_channel_updated(&channel);
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelPermission => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::SetChannelOwner(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let mut channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::SetChannelPermission(new_permission) = unwrap!(call.params()) {
                    channel.set_permission(new_permission.clone());
                    ua!(self).on_channel_updated(&channel);
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelName => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::SetChannelName(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let mut channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::SetChannelName(name) = unwrap!(call.params()) {
                    channel.set_name(name.as_str());
                    ua!(self).on_channel_updated(&channel);
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelNotice => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::SetChannelNotice(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let mut channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::SetChannelNotice(notice) = unwrap!(call.params()) {
                    channel.set_notice(notice.as_str());
                    ua!(self).on_channel_updated(&channel);
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelRole => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::SetChannelMemberRole(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::SetChannelMemberRole(member_role) = unwrap!(call.params()) {
                    let role = member_role.role();
                    let changed_members = member_role.members().iter()
                        .map(|id| match channel.member(id) {
                            Some(m) => m,
                            None => channel::Member::unknown(id)
                        })
                        .collect::<Vec<channel::Member>>();
                    ua!(self).on_channel_members_role_changed(&channel, changed_members.as_ref(), role);
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelBan => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::BanChannelMembers(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::BanChannelMembers(ids) = unwrap!(call.params()) {
                    let changed = ids.iter().map(|id| match channel.member(id) {
                            Some(m) => m,
                            None => channel::Member::unknown(id)
                        })
                        .collect::<Vec<channel::Member>>();
                    ua!(self).on_channel_members_banned(&channel, changed.as_ref());
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelUnban => {
                let complete = |rc: Result<()>| {
                    if let Some(Promise::UnbanChannelMembers(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::UnbanChannelMembers(ids) = unwrap!(call.params()) {
                    let changed = ids.iter().map(|id| match channel.member(id) {
                            Some(m) => m,
                            None => channel::Member::unknown(id)
                        })
                        .collect::<Vec<channel::Member>>();
                    ua!(self).on_channel_members_unbanned(&channel, changed.as_ref());
                }
                complete(Ok(()))
            },
            RPCMethod::ChannelRemove =>{
                let complete = |rc: Result<()>| {
                    if let Some(Promise::RemoveChannelMembers(arc)) = call.promise() {
                        lock!(arc).complete(rc)
                    }
                };
                let err_from = |e: Error| {
                    let estr = format!("Internal error: {e}");
                    warn!("{}", estr);
                    Err(Error::State(estr))
                };
                if let Err(e) = preparsed.result::<bool>() {
                    complete(err_from(e));
                    return;
                }
                let channel = match lock!(self.ua).channel(msg.from()) {
                    Ok(Some(channel)) => channel,
                    Ok(None) => Channel::auto(msg.from()),
                    Err(e) => {
                        complete(err_from(e));
                        return;
                    }
                };
                if let Parameters::RemoveChannelMembers(ids) = unwrap!(call.params()) {
                    let changed = ids.iter().map(|id| match channel.member(id) {
                            Some(m) => m,
                            None => channel::Member::unknown(id)
                        })
                        .collect::<Vec<channel::Member>>();
                    ua!(self).on_channel_members_removed(&channel, changed.as_ref());
                }
                complete(Ok(()))
            },
            _ => {
                error!("Internal Error: invalid RPC call {:?}", call.method());
                return;
            }
        }
    }

    async fn on_notification(&mut self, msg: Msg) {
        let Some(body) = msg.body().filter(|b| !b.is_empty()) else {
            warn!("Empty notification received from {}, ignored", msg.from());
            return;
        };

        {
            use hex::ToHex;
            let body_hex = body.encode_hex::<String>();
            println!("Notification body (hex): {}", body_hex);
        }

        let Ok(mut preparsed) = Notification::from(body) else {
            error!("Error parsing notification from {}, message ignored", msg.from());
            return;
        };

        match preparsed.event() {
            events::USER_PROFILE => {
                let profile = match preparsed.data::<Profile>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing profile data in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                if !self.is_me(&profile.id()) && !profile.is_genuine() {
                    warn!("User updated its profile, but the Profile invalid, ignored");
                    return;
                } else {
                    info!("User updated its profile: {}", profile.id());
                }
                let name = profile.name();
                lock!(self.ua).on_user_profile_changed(name, profile.has_avatar());
            },
            events::CHANNEL_PROFILE => {
                if self.is_me(preparsed.operator()) {
                    return;
                }
                let updated = match preparsed.data::<Channel>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing channel data in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                if let Ok(Some(mut channel)) = lock!(self.ua).channel(msg.to()) {
                    channel.update_channel(&updated);
                    lock!(self.ua).on_channel_deleted(&channel)
                }
            },
            events::CHANNEL_DELETED => {
                if self.is_me(preparsed.operator()) {
                    return;
                }
                if let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) {
                    lock!(self.ua).on_channel_deleted(&channel)
                }
            },
            events::CHANNEL_MEMBER_JOINED => {
                let member = match preparsed.data::<channel::Member>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing channel member data in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                if let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) {
                    lock!(self.ua).on_channel_member_joined(&channel, &member);
                }
            },
            events::CHANNEL_MEMBER_LEFT => {
                let memberid = preparsed.operator();
                let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) else {
                    warn!("No channel {{{}}} found, ignored", msg.to());
                    return;
                };
                let member = match channel.member(&memberid) {
                    Some(m) => m,
                    None => channel::Member::unknown(&memberid)
                };
                lock!(self.ua).on_channel_member_left(&channel, &member);
            },
            events::CHANNEL_MEMBERS_ROLE => {
                if self.is_me(preparsed.operator()) {
                    return;
                }
                let updated = match preparsed.data::<ChannelMembersRoleUpdated>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing channel members role data in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                let role = updated.role();
                let ids = updated.members();
                let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) else {
                    warn!("No channel {{{}}} found, ignored", msg.to());
                    return;
                };
                let members = ids.iter()
                    .map(|id| match channel.member(id) {
                        Some(m) => m,
                        None => channel::Member::unknown(id)
                    })
                    .collect::<Vec<channel::Member>>();
                lock!(self.ua).on_channel_members_role_changed(&channel, members.as_ref(), role);
            },
            events::CHANNEL_MEMBERS_BANNED => {
                if self.is_me(preparsed.operator()) {
                    return;
                }
                let ids = match preparsed.data::<Vec<Id>>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing channel members IDs in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) else {
                    warn!("No channel {{{}}} found, ignored", msg.to());
                    return;
                };
                let members = ids.iter()
                    .map(|id| match channel.member(id) {
                        Some(m) => m,
                        None => channel::Member::unknown(id)
                    })
                    .collect::<Vec<channel::Member>>();
                lock!(self.ua).on_channel_members_banned(&channel, members.as_ref());
            },
            events::CHANNEL_MEMBERS_UNBANNED => {
                if self.is_me(preparsed.operator()) {
                    return;
                }
                let ids = match preparsed.data::<Vec<Id>>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing channel members IDs in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) else {
                    warn!("No channel {{{}}} found, ignored", msg.to());
                    return;
                };
                let members = ids.iter()
                    .map(|id| match channel.member(id) {
                        Some(m) => m,
                        None => channel::Member::unknown(id)
                    })
                    .collect::<Vec<channel::Member>>();
                lock!(self.ua).on_channel_members_unbanned(&channel, members.as_ref());
            },
            events::CHANNEL_MEMBERS_REMOVED => {
                if self.is_me(preparsed.operator()) {
                    return;
                }
                let ids = match preparsed.data::<Vec<Id>>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Error parsing channel members IDs in notification from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };
                let Ok(Some(channel)) = lock!(self.ua).channel(msg.to()) else {
                    warn!("No channel {{{}}} found, ignored", msg.to());
                    return;
                };
                let members = ids.iter()
                    .map(|id| match channel.member(id) {
                        Some(m) => m,
                        None => channel::Member::unknown(id)
                    })
                    .collect::<Vec<channel::Member>>();
                lock!(self.ua).on_channel_members_removed(&channel, members.as_ref());
            },
            _ => {
                error!("Internal Error: invalid notification {:?}, ignored", preparsed.event());
                return;
            }
        }
    }

    async fn on_rpc_request(&mut self, msg: Msg) {
        let Some(body) = msg.body().filter(|b| !b.is_empty()) else {
            warn!("Empty RPC request received from {}, ignored", msg.from());
            return;
        };

        if msg.has_original_body() {
            // TODO
            return;
        }
        {
            use hex::ToHex;
            let body_hex = body.encode_hex::<String>();
            println!("RPC response body (hex): {}", body_hex);
        }

        let Ok(preparsed) = RPCRequest::from(body) else {
            error!("Failed to parse RPC response from {}, message ignored", msg.from());
            return;
        };
        match preparsed.method() {
            RPCMethod::DeviceList   => {},  // ignored
            RPCMethod::DeviceRevoke => {}   // ignored
            RPCMethod::ContactPush  => {},
            RPCMethod::ContactClear => {},
            RPCMethod::ChannelCreate => {},
            RPCMethod::ChannelRemove => {},
            RPCMethod::ChannelJoin  => {},
            RPCMethod::ChannelLeave => {},
            RPCMethod::ChannelOwner => {},
            RPCMethod::ChannelPermission => {},
            RPCMethod::ChannelName  => {},
            RPCMethod::ChannelNotice => {},
            RPCMethod::ChannelRole  => {},
            RPCMethod::ChannelBan   => {},
            RPCMethod::ChannelUnban => {},
            _ => {
                error!("Unknown RPC method in response from {}, ignored", msg.from());
                return;
            }
        }
    }

    #[allow(unused)]
    async fn try_refresh_profile(&self, _id: &Id) -> Result<Profile> {
        unimplemented!()
    }
}
