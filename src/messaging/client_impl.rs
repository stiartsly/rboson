use std::collections::HashMap;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;
use serde::{Serialize, de::DeserializeOwned};
use serde_cbor;
use sha2::{Digest, Sha256};
use url::Url;
use log::{error, warn, info, debug};
use tokio::task::JoinHandle;
use md5;
use rumqttc::{
    MqttOptions,
    AsyncClient,
    SubscribeFilter,
    Event,
    Packet,
    Outgoing //, Incoming
};

use crate::{
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
    UserAgent,
    DefaultUserAgent,
    InviteTicket,
    Contact,
    ClientBuilder,
    MessagingClient,
    ConnectionListener,
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
        parameters,
        promise::{self, Ack, Promise, Waiter},
    },
    message::{
        Message,
        MessageType,
        MessageBuilder
    }
};

#[allow(dead_code)]
pub struct Client {
    peer            : PeerInfo,
    user            : CryptoIdentity,
    device          : CryptoIdentity,
    client_id       : String,

    inbox           : String,
    outbox          : String,
    broadcast       : String,

    service_info    : Option<api_client::MessagingServiceInfo>,

    server_context  : Option<CryptoContext>,
    self_context    : Option<CryptoContext>,

    api_client      : Option<APIClient>,
    disconnect      : bool,

    worker_task     : Option<JoinHandle<()>>,
    worker_client   : Option<Arc<Mutex<AsyncClient>>>,

    pending_calls   : HashMap<Id, Arc<Mutex<RPCRequest>>>,

    user_agent      : Arc<Mutex<DefaultUserAgent>>,
}

#[allow(dead_code)]
impl Client {
    pub(crate) fn new(b: ClientBuilder) -> Result<Self> {
        let user_agent = b.ua();
        let ua = user_agent.lock().unwrap();
        if !ua.is_configured() {
            drop(ua);
            return Err(Error::State("User agent is not configured".into()));
        }

        let peer = ua.peer().clone();
        let user = ua.user().unwrap().identity().clone();
        let device = ua.device().unwrap().identity().unwrap().clone();

        drop(ua);
        user_agent.lock().unwrap().harden();

        let clientid = bs58::encode({
            md5::compute(device.id().as_bytes()).0
        }).into_string();

        let bs58_id = user.id().to_base58();
        Ok(Self {
            peer,
            user,
            device,
            service_info    : None,

            client_id       : clientid,
            inbox           : format!("inbox/{bs58_id}",),
            outbox          : format!("outbox/{bs58_id}",),
            broadcast       : "broadcast".into(),

            api_client      : None,
            disconnect      : false,

            worker_client   : None,
            worker_task     : None,

            self_context    : None,
            server_context  : None,

            pending_calls   : HashMap::new(),

            user_agent      : user_agent.clone(),
        })
    }

    fn ua(&self) -> &Arc<Mutex<DefaultUserAgent>> {
        &self.user_agent
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

    fn self_ctxt(&self) -> &CryptoContext {
        self.self_context
            .as_ref()
            .expect("Self crypto context should be created")
    }

    fn server_ctxt(&self) -> &CryptoContext {
        self.server_context
            .as_ref()
            .expect("Server crypto context should be created")
    }


    pub(crate) fn next_index(&mut self) -> i32 {
        0 // TODO
    }

    pub fn load_access_token(&mut self) -> Result<Option<String>> {
        // TODO:
        Ok(Some("TODO".into()))
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Messaging client Started!");

        _ = self.load_access_token()?;

        let ua = self.user_agent.lock().unwrap();
        let peer = ua.peer().clone();
        let user = ua.user().unwrap().identity().clone();
        let device = ua.device().unwrap().identity().unwrap().clone();
        let api_url = match peer.alternative_url().as_ref() {
            None => Err(Error::State("Alternative URL should be set".into())),
            Some(url) => Url::parse(url).map_err(|e|
                Error::State(format!("Failed to parse API URL: {e}"))
            )
        }?;
        drop(ua);

        let api_client = api_client::Builder::new()
            .with_base_url(&api_url)
            .with_home_peerid(peer.id())
            .with_user_identity(&user)
            .with_device_identity(&device)
            //.with_access_token(self.load_access_token().ok())
            .with_access_token_refresh_handler(|_| {})
            .build()?;

        self.api_client = Some(api_client);

        let ver_id = match self.ua().lock().unwrap().contact_version() {
            Ok(v) => v,
            Err(e) => {
                warn!("Fetching all contacts due to failure to retrieve contacts version from local agent: {}", e);
                None
            }
        };

        if ver_id.is_none() {
            let mut update = self.api_client().fetch_contacts_update(
                ver_id.as_deref()
            ).await?;

            if let Some(version_id) = update.version_id() {
                _ = self.ua().lock().unwrap().put_contacts_update(
                    &version_id,
                    update.contacts().as_slice()
                ).map_err(|e|{
                    warn!("Failed to put contacts update: {}, ignore and continue", e);
                });
            }
        }

        self.service_info = Some(self.api_client().service_info().await?);
        self.self_context = Some(self.user.create_crypto_context(user.id())?);
        self.server_context = Some(self.user.create_crypto_context(peer.id())?);

        Ok(())
    }

    pub async fn stop(&mut self, forced: bool) {
        _ = self.disconnect().await;
        self.disconnect = true;
        // TODO: self.server_context.close();

        self.server_context = None;
        self.self_context = None;

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

    pub async fn connect(&mut self) -> Result<()> {
        MessagingClient::connect(self).await
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

    async fn sign_into_invite_ticket(&self, channel_id: &Id, invitee: Option<&Id>) -> Result<InviteTicket> {
        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        use std::time::{SystemTime, Duration};
        let expire = SystemTime::now() + Duration::from_secs(InviteTicket::EXPIRATION);
        let expire_ts = expire.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();


        let mut sha256 = Sha256::new();
        sha256.update(channel_id.as_bytes());
        sha256.update(self.user.id().as_bytes());
        if let Some(invitee) = invitee {
            sha256.update(invitee.as_bytes());
        } else {
            sha256.update(Id::max().as_bytes());
        }

        // sha256.update(expire.duration_since(SystemTime::UNIX_EPOCH)?.as_secs().to_be_bytes());
        // TODO: expire.

        let sig = self.user.sign_into(sha256.finalize().as_slice())?;
        let sk = channel.session_keypair().unwrap().private_key().as_bytes();
        let sk = match invitee {
            Some(invitee) => self.user.encrypt_into(invitee, sk)?,
            None => sk.to_vec(),
        };

        Ok(InviteTicket::new(
            channel_id.clone(),
            self.user.id().clone(),
            invitee.is_none(),
            expire_ts,
            sig,
            Some(sk)
        ))
    }

    async fn send_rpc_request(
        &mut self,
        recipient: &Id,
        request: RPCRequest
    ) -> Result<()> {
        let msg = MessageBuilder::new(self, MessageType::Call)
            .with_to(recipient.clone())
            .with_body(serde_cbor::to_vec(&request).unwrap())
            .build()?;

        //self.pending_calls.insert(0, request);
        self.send_message_intenral(msg).await
    }

    async fn pub_message_internal(&self, msg: Message) -> Result<()> {
        let outbox = self.outbox.as_str();
        let payload = serde_cbor::to_vec(&msg).unwrap();

        self.worker().lock().unwrap().publish(
            outbox,
            rumqttc::QoS::AtLeastOnce,
            false,
            payload
        ).await.map_err(|e| {
            Error::State(format!("Failed to publish message: {}", e))
        })?;

        debug!("Published message to outbox {}", outbox);

        // TODO: pending messages.
        // TODO: sending messages;
        Ok(())
    }

    async fn send_message_intenral(&self, msg: Message) -> Result<()> {
        let Some(body) = msg.body() else {
            return self.pub_message_internal(msg).await
        };

        if body.is_empty() || msg.to() == self.peer.id() {
            return self.pub_message_internal(msg).await
        }

        let encrypted = match msg.message_type() {
            MessageType::Message => {
                let recipient = self.ua().lock().unwrap().contact(msg.to())?;
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

                self.user
                    .create_crypto_context(&sid)?
                    .encrypt_into(body)?
            },
            MessageType::Call => {
                self.user
                    .create_crypto_context(msg.to())?
                    .encrypt_into(body)?
            },
            _ => {
                let estr = format!("INTERNAL error: unsupported message type {:?}", msg.message_type());
                error!("{}", estr);
                return Err(Error::State(estr));
            }
        };

        self.pub_message_internal(msg.dup_from(encrypted)).await
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
                    Event::Incoming(packet) => worker.on_incoming_msg(packet),
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
        self.user_agent.lock().unwrap().on_connecting();

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
            self.user_agent.lock().unwrap().on_disconnected();
            Error::State(errstr)
        })
    }

    async fn push_contacts_update(&mut self, _updated_contacts: Vec<Contact>) -> Result<()> {
        unimplemented!()
    }

    pub async fn create_channel(
        &mut self,
        permission: Option<channel::Permission>,
        name: &str,
        notice: Option<&str>
    ) -> Result<Channel> {
        MessagingClient::create_channel(self, permission, name, notice).await
    }
}

unsafe impl Send for Client {}

#[allow(dead_code)]
impl MessagingClient for Client {
    fn userid(&self) -> &Id {
        self.user.id()
    }

    fn user_agent(&self) -> Arc<Mutex<dyn UserAgent>> {
        self.user_agent.clone()
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        self.do_connect().await
    }

    async fn disconnect(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn is_connected(&self) -> bool {
        unimplemented!()
    }

    /*
    fn message(&mut self) -> MessageBuilder {
        MessageBuilder::new(self, MessageType::Message)
    }
    */

    async fn update_profile(
        &mut self,
        name: Option<&str>,
        avatar: bool
    ) -> Result<()> {
        let name = name.map(|n| n.nfc().collect::<String>());
        self.api_client().update_profile(
            name.as_deref(),
            avatar
        ).await
    }

    async fn upload_avatar(
        &mut self,
        content_type: &str,
        avatar: &[u8]
    ) -> Result<String> {
        self.api_client().upload_avatar(content_type, avatar).await
    }

    async fn upload_avatar_from_file(
        &mut self,
        content_type: &str,
        file_name: &str
    ) -> Result<String> {
        self.api_client().upload_avatar_from_file(
            content_type,
            file_name.into()
        ).await
    }

    async fn devices(&mut self) -> Result<Vec<ClientDevice>> {
        let ack = Arc::new(Mutex::new(
            promise::DeviceListAck::new())
        );
        let promise = Promise::DeviceList(ack.clone());
        let _req = RPCRequest::new::<_>(
            self.next_index(),
            RPCMethod::DeviceList,
            None::<parameters::ChannelCreate>,
        ).with_promise(promise.clone());

        //self.send_rpc_request(&self.peer.id(), request).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn revoke_device(
        &mut self,
        device_id: &Id
    ) -> Result<()> {
        let ack = Arc::new(Mutex::new(promise::RevokeDeviceAck::new()));
        let promise = Promise::RevokeDevice(ack.clone());
        let req = RPCRequest::new::<Id>(
            self.next_index(),
            RPCMethod::DeviceRevoke,
            Some(device_id.clone())
        ).with_promise(promise.clone());

        self.send_rpc_request(
            &self.peer.id().clone(),
            req
        ).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn create_channel(
        &mut self,
        permission: Option<channel::Permission>,
        name: &str,
        notice: Option<&str>
    ) -> Result<Channel> {
        let session_keypair = signature::KeyPair::random();
        let session_id: Id = session_keypair.public_key().into();
        let permission = permission.unwrap_or(channel::Permission::OwnerInvite);
        let params = parameters::ChannelCreate::new(
            session_id,
            permission,
            Some(name.into()),
            notice.map(|n| n.into()),
        );

        println!("<create_channel> line >>> {}", line!());
        let ack = Arc::new(Mutex::new(promise::CreateChannelAck::new()));
        let promise = Promise::CreateChannel(ack.clone());
        let cookie = self.self_context.as_mut().unwrap().encrypt_into(
            session_keypair.private_key().as_bytes()
        )?;
        let req = RPCRequest::new::<parameters::ChannelCreate>(
            self.next_index(),
            RPCMethod::ChannelCreate,
            Some(params)
        )
        .with_promise(promise.clone())
        .with_cookie(cookie);

        println!("<create_channel> line >>> {}", line!());

        //self.pending_calls.insert(session_id.clone(), req);

        let peerid = self.service_info.as_ref().unwrap().peerid().clone();
        self.send_rpc_request(&peerid, req).await.map_err(|e| {
            println!("Failed to send rpc request to create channel: {}", e);
            e
        })?;

        println!("<create_channel> line >>> {}", line!());

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn remove_channel(
        &mut self,
        channel_id: &Id
    ) -> Result<()> {
        let ack = Arc::new(Mutex::new(promise::RemoveChannelAck::new()));
        let promise = Promise::RemoveChannel(ack.clone());
        let req = RPCRequest::new::<()>(
            self.next_index(),
            RPCMethod::ChannelRemove,
            None
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn join_channel(
        &mut self,
        ticket: &InviteTicket
    ) -> Result<()> {
        if ticket.session_key().is_none() {
            Err(Error::Argument("Invite ticket does not contain session key".into()))?
        }
        if ticket.is_expired() {
            Err(Error::Argument("Invite ticket is expired".into()))?
        }
        if ticket.is_valid(self.user.id()) {
            Err(Error::Argument("Invite ticket is not valid for this user".into()))?
        }

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

        let ack = Arc::new(Mutex::new(promise::JoinChannelAck::new()));
        let promise = Promise::JoinChannel(ack.clone());
        let req = RPCRequest::new::<InviteTicket>(
            self.next_index(),
            RPCMethod::ChannelJoin,
            Some(ticket.proof().clone())
        )
        .with_promise(promise.clone())
        .with_cookie({
            // TODO: session_key.
            Vec::<u8>::new() // TODO
        });
        self.send_rpc_request(ticket.channel_id(), req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn leave_channel(&mut self, channel_id: &Id) -> Result<()> {
        let ack = Arc::new(Mutex::new(promise::LeaveChannelAck::new()));
        let promise = Promise::LeaveChannel(ack.clone());
        let req = RPCRequest::new::<()>(
            self.next_index(),
            RPCMethod::ChannelLeave,
            None
        ).with_promise(promise.clone());
        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn create_invite_ticket(
        &mut self,
        channel_id: &Id,
        invitee: Option<&Id>
    ) -> Result<InviteTicket> {
        self.sign_into_invite_ticket(channel_id, invitee).await
    }

    async fn set_channel_owner(
        &mut self,
        channel_id: &Id,
        new_owner: &Id
    ) -> Result<()> {
        let ua = self.ua().lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };
        if !channel.is_owner(self.user.id()) {
            Err(Error::State("Not channel owner".into()))?
        }
        if channel.is_member(new_owner) {
            Err(Error::State("New owner is not in the channel".into()))?
        }
        drop(ua);

        let ack = Arc::new(Mutex::new(promise::SetChannelOwnerAck::new()));
        let promise = Promise::SetChannelOwner(ack.clone());
        let req = RPCRequest::new::<Id>(
            self.next_index(),
            RPCMethod::ChannelOwner,
            Some(new_owner.clone())
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_permission(
        &mut self,
        channel_id: &Id,
        permission: Permission
    ) -> Result<()> {
        let ua = self.ua().lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };
        if !channel.is_owner(self.user.id()) {
            Err(Error::State("Not channel owner".into()))?
        }
        drop(ua);

        let ack = Arc::new(Mutex::new(promise::SetChannelPermAck::new()));
        let promise = Promise::SetChannelPerm(ack.clone());
        let req = RPCRequest::new::<Permission>(
            self.next_index(),
            RPCMethod::ChannelPermission,
            Some(permission)
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_name(
        &mut self,
        channel_id: &Id,
        name: Option<&str>
    ) -> Result<()> {
        let ua = self.ua().lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let name = name.map(|n|
            match !n.is_empty() {
                true => Some(n.nfc().collect::<String>()),
                false => None,
            }
        ).flatten();

        let ack = Arc::new(Mutex::new(promise::SetChannelNameAck::new()));
        let promise = Promise::SetChannelName(ack.clone());
        let req = RPCRequest::new::<String>(
            self.next_index(),
            RPCMethod::ChannelName,
            name
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_notice(
        &mut self,
        channel_id: &Id,
        notice: Option<&str>
    ) -> Result<()> {
        let ua = self.ua().lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let notice = notice.map(|n|
            match n.is_empty() {
                true => None,
                false => Some(n.nfc().collect::<String>())
        }).flatten();

        let ack = Arc::new(Mutex::new(promise::SetChannelNoticeAck::new()));
        let promise = Promise::SetChannelNotice(ack.clone());
        let req = RPCRequest::new::<String>(
            self.next_index(),
            RPCMethod::ChannelNotice,
            notice
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn set_channel_member_role(
        &mut self,
        channel_id: &Id,
        members: Vec<&Id>,
        role: Role
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let ua = self.ua().lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };
        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let role = parameters::ChannelMemberRole::new(
            members.into_iter().map(|id| id.clone())
                .collect::<Vec<Id>>(),
            role,
        );

        let ack = Arc::new(Mutex::new(promise::SetChannelMemberRoleAck::new()));
        let promise = Promise::SetChannelMemberRole(ack.clone());
        let req = RPCRequest::new::<parameters::ChannelMemberRole>(
            self.next_index(),
            RPCMethod::ChannelRole,
            Some(role)
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn ban_channel_members(
        &mut self,
        channel_id: &Id,
        members: Vec<&Id>,
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(())
        }

        let ua = self.ua().lock().unwrap();
        let Some(ch) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };

        let userid = self.user.id();
        if !ch.is_owner(userid) && !ch.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let members = members.into_iter()
            .map(|id| id.clone())
            .collect::<Vec<Id>>();

        let ack = Arc::new(Mutex::new(promise::BanChannelMembersAck::new()));
        let promise = Promise::BanChannelMembers(ack.clone());
        let req = RPCRequest::new::<Vec<Id>>(
            self.next_index(),
            RPCMethod::ChannelBan,
            Some(members)
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn unban_channel_members(
        &mut self,
        channel_id: &Id,
        members: Vec<&Id>
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let ua = self.ua().lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };

        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let members = members.into_iter()
            .map(|id| id.clone())
            .collect::<Vec<Id>>();

        let ack = Arc::new(Mutex::new(promise::UnbanChannelMembersAck::new()));
        let promise = Promise::UnbanChannelMembers(ack.clone());
        let req = RPCRequest::new::<Vec<Id>>(
            self.next_index(),
            RPCMethod::ChannelUnban,
            Some(members)
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn remove_channel_members(
        &mut self,
        channel_id: &Id,
        members: Vec<&Id>
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let ua = self.user_agent.lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };

        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let members = members.into_iter()
            .map(|id| id.clone())
            .collect::<Vec<Id>>();
        let ack = Arc::new(Mutex::new(promise::RemoveChannelMembersAck::new()));
        let promise = Promise::RemoveChannelMembers(ack.clone());
        let req = RPCRequest::new::<Vec<Id>>(
            self.next_index(),
            RPCMethod::ChannelRemove,
            Some(members)
        ).with_promise(promise.clone());

        self.send_rpc_request(channel_id, req).await?;

        match Waiter::new(promise).await {
            Ok(_) => ack.lock().unwrap().result(),
            Err(e) => Err(e)
        }
    }

    async fn add_contact(
        &mut self,
        id: &Id,
        home_peer_id: Option<&Id>,
        session_key: &[u8],
        remark: Option<&str>
    ) -> Result<()> {

        // check the session key
        _ = signature::KeyPair::try_from(
            session_key
        ).map_err(|_| {
            Error::Argument(format!("Invalid contact private key"))
        })?;

        let contact = Contact::new1(
            id.clone(),
            home_peer_id.cloned(),
            session_key.to_vec(),
            remark.map(|r| r.nfc().collect::<String>())
        )?;

        self.push_contacts_update(vec![contact]).await
    }

    async fn contact(&self, id: &Id) -> Result<Option<Contact>> {
        let ua = self.user_agent.clone();
        let id = id.clone();

        match tokio::spawn(async move {
            ua.lock().unwrap().contact(&id)
        }).await {
            Ok(contact) => contact,
            Err(e) => Err(Error::State(format!("Failed to get contact: {}", e)))
        }
    }

    async fn channel(&self, id: &Id) -> Result<Option<Channel>> {
        let ua = self.user_agent.clone();
        let id = id.clone();

        match tokio::spawn(async move {
            ua.lock().unwrap()
                .channel(&id)
        }).await {
            Ok(channel) => channel,
            Err(e) => Err(Error::State(format!("Failed to get channel: {}", e)))
        }
    }

    async fn contacts(&self) -> Result<Vec<Contact>> {
        let ua = self.user_agent.clone();

        match tokio::spawn(async move {
            ua.lock().unwrap().contacts()
        }).await {
            Ok(contacts) => contacts,
            Err(e) => Err(Error::State(format!("Failed to get contact: {}", e)))
        }
    }

    async fn update_contact(&mut self, contact: Contact) -> Result<()> {
        if !contact.is_modified() {
            Err(Error::Argument("Contact is not modified".into()))?;
        }
        self.push_contacts_update(vec![contact.clone()]).await
    }

    async fn remove_contact(&mut self, _id: &Id) -> Result<()> {
        let mut ua = self.ua().lock().unwrap();
        let Some(mut contact) = ua.contact(_id)? else {
            return Err(Error::State("Contact does not exist".into()));
        };

        if contact.is_auto() {
            ua.remove_contacts(vec![_id])?;
            return Ok(())
        }
        drop(ua);

        if contact.is_deleted() {
            return Ok(())
        }
        contact.set_deleted(true);
        self.push_contacts_update(vec![contact]).await
    }

    async fn remove_contacts(&mut self, _ids: Vec<&Id>) -> Result<()> {
        unimplemented!()
    }
}

#[allow(unused)]
struct MessagingWorker {
    user_agent      : Arc<Mutex<DefaultUserAgent>>,
    worker_client   : Arc<Mutex<AsyncClient>>,


    connect_promise : Option<Box<dyn FnMut() + Send + Sync>>,

    server_context  : Option<CryptoContext>,    // TODO,
    disconnect      : bool,
    failures        : u32,


    peer            : PeerInfo,

    inbox           : String,
    outbox          : String,
    broadcast       : String,

    pending_calls   : HashMap<u32, RPCRequest>,

    user            : Option<CryptoIdentity>,
}

#[allow(unused)]
impl MessagingWorker {
    #[allow(unused)]
    fn new(client: &Client,  mqttc: Arc<Mutex<AsyncClient>>) -> Self {
        Self {
            user_agent      : client.ua().clone(),
            worker_client   : mqttc,

            connect_promise : None,
            server_context  : None,
            disconnect      : false,
            failures        : 0,
            peer            : client.peer.clone(),

            inbox           : client.inbox.clone(),
            outbox          : client.outbox.clone(),
            broadcast       : client.broadcast.clone(),

            pending_calls   : HashMap::new(),
            user            : None,
        }
    }

    fn ua(&self) -> &Arc<Mutex<DefaultUserAgent>> {
        &self.user_agent
    }

    fn is_me(&self, _id: &Id) -> bool {
        true // TODO
    }

    fn user_mut(&mut self) -> &mut CryptoIdentity {
        self.user.as_mut()
            .expect("User identity should be set")
    }

    fn on_incoming_msg(&mut self, packet: Packet) {
        match packet {
            Packet::Publish(publish) => self.on_pub_msg(publish),
            Packet::PubAck(_)   => {},
            Packet::SubAck(_)   => {},
            Packet::UnsubAck(_) => {},
            Packet::Disconnect  => self.on_close(),
            Packet::PingResp    => self.on_ping_response(),
            Packet::ConnAck(_)  => {},

            _ => info!("Unknown MQTT event: {:?}", packet)
        }
    }

    fn on_outgoing_msg(&mut self, _packet: Outgoing) {
        match _packet {
            Outgoing::Publish(_pktid) => {},
            _ => {},
        }
    }

    fn on_close(&mut self) {
        if self.disconnect {
            self.ua().lock().unwrap().on_disconnected();
            info!("disconnected!");
            return;
        }

        self.failures += 1;
        error!("Connection lost, attempt to reconnect in {} seconds...", 5); // TODO:

    }

    fn on_ping_response(&mut self) {
        info!("Ping response received");
    }

    fn on_pub_msg(&mut self, publish: rumqttc::Publish) {
        let topic = publish.topic.as_str();
        debug!("Got message on topic: {}", topic);

        println!("<<on_pub_msg>> line: {}", line!());
        let payload = self.server_context.as_ref().unwrap().decrypt_into(
            &publish.payload
        );
        println!("<<on_pub_msg>> line: {}", line!());
        let Ok(payload) = payload  else {
            error!("Failed to decrypt message payload from {topic}, ignored");
            return;
        };

        let msg = serde_cbor::from_slice::<Message>(&payload);
        let Ok(mut msg) = msg else {
            error!("Failed to deserialize message from {topic}, ignored");
            return;
        };
        if msg.is_valid() {
            error!("Received invalid message from {topic}, ignored");
            return;
        }
        msg.mark_encrypted(true);

        if topic == self.inbox {
            self.on_inbox_message(msg);
        } else if topic == self.outbox {
            self.on_outbox_message(msg);
        } else if topic == self.broadcast {
            self.on_broadcast_message(msg);
        } else {
            error!("Received message with unknown topic: {topic}, ignored");
            return;
        }
    }

    fn on_inbox_message(&mut self, mut msg: Message) {
        let Some(body) = msg.body() else {
            msg.mark_encrypted(false);
            return self.pub_msg_internal(msg);
        };

        if body.is_empty() || msg.from() == self.peer.id() {
            msg.mark_encrypted(false);
            return self.pub_msg_internal(msg);
        }

        if self.is_me(msg.to()){
            match msg.message_type() {
                MessageType::Message => {
                    // Message: sender -> me
                    // The body is encrypted using the sender's private key
                    // and the session public key associated with that sender.

                    let sender = self.user_agent.lock().unwrap().contact(msg.from());
                    let Ok(Some(sender)) = sender else {
                        warn!("Failed to get contact info for sender {}", msg.from());
                        return;
                    };

                    if sender.has_session_key() {
                        msg.decrypt_body(
                            sender.rx_crypto_context().unwrap(),
                        ).unwrap_or_else(|e| {
                            error!("Failed to decrypt message body: {}, ignored", e);
                        });
                    }
                },
                MessageType::Call => {
                    // Call: sender(user | channel) -> me
                    // The body is encrypted using the sender's private key
                    // and my public key.

                    msg.decrypt_body(
                        &self.user_mut().create_crypto_context(msg.from()).unwrap(),
                    ).unwrap_or_else(|e| {
                        error!("Failed to decrypt call body: {}, ignored", e);
                    });
                },
                _ => {}
            }
        } else {
            let channel = self.ua().lock().unwrap().channel(msg.to());
            let Ok(Some(channel)) = channel else {
                return self.pub_msg_internal(msg);
            };

            let Some(kp) = channel.session_keypair() else {
                return self.pub_msg_internal(msg);
            };

            match msg.message_type() {
                MessageType::Message => {
                    // Message: sender -> channel
                    // The body is encrypted using the sender's private key
                    // and the session public key of channel.
                    //TODO:

                    let ctxt = channel.rx_crypto_context(msg.from());
                    msg.decrypt_body(ctxt);
                },
                MessageType::Notification => {
                    // Message: channel -> channel
                    // The body is encrypted using the channel's private key
                    // and the channel session's public key

                    let ctxt = channel.rx_crypto_context1();
                    // TODO
                },
                _ => {}
            }
        }

        self.pub_msg_internal(msg)
    }

    fn on_outbox_message(&mut self, _message: Message) {
        println!(">>>> TODO:");
    }

    fn on_broadcast_message(&mut self, mut msg: Message) {
        // Broadcast notifications from the service peer.
		// Message body is encrypted with the message envelope,
		// it was already decrypted here

        msg.mark_encrypted(false);
        self.ua().lock().unwrap().on_broadcast(msg);
    }

    fn on_msg_internal(&mut self, msg: Message) {
        let conv_id = match !self.is_me(msg.to()) {
            true => msg.to().clone(),
            false => msg.from().clone()
        };

        self.ua().lock().unwrap().on_message(msg);

        let mut need_refresh = false;
        let contact = self.ua().lock().unwrap().contact(&conv_id);
        if let Ok(Some(contact)) = contact {
            if contact.is_staled() {
                need_refresh = true;
            }
        }else {
            need_refresh = true;
        }

        if need_refresh {
            //self.arefresh_profile(&conv_id).await.on_success(|profile| {
            //    self.ua().lock().unwrap().on_contact_profile(&conv_id, profile);
            //});
        }
    }

    fn on_rpc_response(&mut self, msg: Message) {
        let Some(body) = msg.body() else {
            error!("Message body is missing in RPC response from {}, ignored", msg.from());
            return;
        };

        if body.is_empty() {
            error!("Empty message body in RPC response from {}, ignored", msg.from());
            return;
        }

        let Ok(mut preparsed) = RPCResponse::from(body) else {
            error!("Failed to parse RPC response from {}, ignored", msg.from());
            return;
        };

        let Some(mut request) = self.pending_calls.remove(preparsed.id()) else {
            error!("Unmatched RPC response id {} from {}, ignored", preparsed.id(), msg.from());
            return;
        };

        match request.method() {
            RPCMethod::DeviceList => {
                request.complete::<Vec<ClientDevice>>(preparsed);
            },

            RPCMethod::DeviceRevoke => {
                request.complete::<()>(preparsed);
            },

            RPCMethod::ContactPush => {
                request.complete::<()>(preparsed);
            },

            RPCMethod::ContactClear => { // TODO:
            },

            RPCMethod::ChannelCreate => { // TODO:
            },
            RPCMethod::ChannelRemove => { // TODO:
            },
            RPCMethod::ChannelJoin => { // TODO:
            },
            RPCMethod::ChannelLeave => { // TODO:
            },
            RPCMethod::ChannelOwner => { // TODO:
            },
            RPCMethod::ChannelPermission => { // TODO:
            },
            RPCMethod::ChannelName => { // TODO:
            },
            RPCMethod::ChannelNotice => { // TODO:
            },
            RPCMethod::ChannelRole => { // TODO:
            },
            RPCMethod::ChannelBan => { // TODO:
            },
            RPCMethod::ChannelUnban => { // TODO:
            },
            _ => {
                error!("Unknown RPC method in response from {}, ignored", msg.from());
                return;
            }
        }
        unimplemented!()
    }

    #[allow(unused)]
    fn on_rpc_request<P,R>(&mut self, msg: Message) where P: Serialize, R: DeserializeOwned{
        if msg.body_is_empty() {
            error!("Empty RPC response message from {}, ignored", msg.from());
            return;
        }

        if msg.has_original_body() {
            // TODO
        }

        let request: Option<RPCRequest> = None; // TODO:
        match request.unwrap().method() {
            RPCMethod::DeviceList => {
                /*
                let call: RPCRequest<(), Vec<ClientDevice>> = request.unwrap();
                let response: RPCResponse<Vec<ClientDevice>> = match serde_cbor::from_slice(msg.body().unwrap()) {
                    Ok(resp) => resp,
                    Err(e) => {
                        error!("Failed to parse RPC response from {}: {}, ignored", msg.from(), e);
                        return;
                    }
                };

                if response.is_error() {
                    error!("RPC response from {} error: {}, ignored", msg.from(), response.error().unwrap());
                    return;
                }

                let devices = response.result().unwrap_or_else(|| {
                    vec![]
                });
                self.user_agent.lock().unwrap().on_device_list(devices);
                */
            },
            RPCMethod::DeviceRevoke => {// TODO:
            },
            RPCMethod::ContactPush => { // TODO:
            },
            RPCMethod::ContactClear => { // TODO:
            },

            RPCMethod::ChannelCreate => { // TODO:
            },
            RPCMethod::ChannelRemove => { // TODO:
            },
            RPCMethod::ChannelJoin => { // TODO:
            },
            RPCMethod::ChannelLeave => { // TODO:
            },
            RPCMethod::ChannelOwner => { // TODO:
            },
            RPCMethod::ChannelPermission => { // TODO:
            },
            RPCMethod::ChannelName => { // TODO:
            },
            RPCMethod::ChannelNotice => { // TODO:
            },
            RPCMethod::ChannelRole => { // TODO:
            },
            RPCMethod::ChannelBan => { // TODO:
            },
            RPCMethod::ChannelUnban => { // TODO:
            },
            _ => {
                error!("Unknown RPC method in response from {}, ignored", msg.from());
                return;
            }
        }
        unimplemented!()
    }

    fn on_notification(&mut self, _message: Message) {
        unimplemented!()
    }

    fn pub_msg_internal(&mut self, msg: Message) {
        match msg.message_type() {
            MessageType::Message => {
                self.on_msg_internal(msg);
            },
            MessageType::Call => {
                self.on_rpc_response(msg);
            },
            MessageType::Notification => {
                self.on_notification(msg);
            }
        }
    }

    #[allow(unused)]
    async fn try_refresh_profile(&self, _id: &Id) -> Result<Profile> {
        unimplemented!()
    }
}
