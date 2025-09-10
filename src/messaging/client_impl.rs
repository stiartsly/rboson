use std::collections::HashMap;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;
use serde::Serialize;
use serde_cbor;
use sha2::{Digest, Sha256};
use url::Url;
use log::{debug, info, error};
use tokio::task::JoinHandle;
use md5;
use rumqttc::{
    MqttOptions,
    AsyncClient,
    SubscribeFilter,
    Event,
    Packet
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
    InviteTicket,
    Contact,
    ClientBuilder,
    MessagingClient,
    api_client::{self, APIClient},
    channel::{Role, Permission, Channel},
    rpc::request::RPCRequest,
    rpc::method::RPCMethod,
    message::{Message, MessageType, MessageBuilder},
    channel,
    rpc::parameters
};

#[allow(dead_code)]
pub struct Client {
    peer            : PeerInfo,
    user            : CryptoIdentity,
    device          : CryptoIdentity,
    client_id       : String,

    inbox           : String,
    outbox          : String,

    service_info    : Option<api_client::MessagingServiceInfo>,

    server_context  : CryptoContext,
    self_context    : CryptoContext,

    api_client      : APIClient,
    disconnect      : bool,


    mqttc_task      : Option<JoinHandle<()>>,
    mqttc_client    : Option<AsyncClient>,
    mqttc_handler   : Option<Arc<Mutex<MqttcHandler>>>,

    pending_msgs    : Arc<Mutex<HashMap<u16, Message>>>,
    user_agent      : Arc<Mutex<dyn UserAgent>>,
}

#[allow(dead_code)]
impl Client {
    pub(crate) fn new(b: ClientBuilder) -> Result<Self> {
        let user_agent = b.user_agent();
        let agent = user_agent.lock().unwrap();
        if !agent.is_configured() {
            Err(Error::State("User agent is not configured".into()))?;
        }

        let peer = agent.peer().clone();
        let user = agent.user().unwrap().identity().clone();
        let device = agent.device().unwrap().identity().unwrap().clone();

        drop(agent);
        user_agent.lock().unwrap().harden();

        let api_client = api_client::Builder::new()
            .with_base_url(b.api_url())
            .with_home_peerid(peer.id())
            .with_user_identity(&user)
            .with_device_identity(&device)
            .with_access_token("TODO")
            .with_access_token_refresh_handler(|_| {})
            .build()?;

        let client_id = bs58::encode({
            md5::compute(device.id().as_bytes()).0
        }).into_string();

        let bs58_userid = user.id().to_base58();
        let self_context = user.create_crypto_context(user.id())?;
        let server_context = device.create_crypto_context(device.id())?;

        Ok(Self {
            peer,
            user,
            device,
            service_info    : None,

            client_id,
            inbox           : format!("inbox/{}", bs58_userid),
            outbox          : format!("outbox/{}", bs58_userid),

            api_client,
            disconnect      : false,

            mqttc_task      : None,
            mqttc_client    : None,
            mqttc_handler   : None,

            self_context,
            server_context,

            pending_msgs    : Arc::new(Mutex::new(HashMap::new())),
            user_agent      : user_agent.clone(),
        })
    }

    fn mqttc(&self) -> &AsyncClient {
        self.mqttc_client
            .as_ref()
            .expect("MQTT client should be created")
    }

    fn mqttc_msg_handler(&self) -> Arc<Mutex<MqttcHandler>> {
        self.mqttc_handler
            .as_ref()
            .expect("MQTT client message handler should be created")
            .clone()
    }

    fn ua(&self) -> Arc<Mutex<dyn UserAgent>> {
        self.user_agent.clone()
    }


    pub(crate) fn next_index(&mut self) -> i32 {
        // TODO
        0
    }

    pub fn load_access_token(&mut self) -> Option<String> {
        Some("TODO".into())
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Messaging client Started!");

        let mut agent = self.user_agent.lock().unwrap();
        let version_id = agent.contact_version()?;

        // TODO: self_context.

        if version_id.is_none() {
            let mut update = self.api_client.fetch_contacts_update(
                version_id.as_ref().map(|v| v.as_str())
            ).await?;

            let Some(version_id) = update.version_id() else {
                Err(Error::State("Contacts update does not contain version id".into()))?
            };

            let contacts = update.contacts();
            agent.put_contacts_update(&version_id, &contacts).map_err(|e|
                Error::State(format!("Failed to put contacts update: {}", e))
            )?;
        }

        let service_info = self.api_client.service_info().await?;
        self.server_context = self.user.create_crypto_context(
            service_info.peerid()
        )?;

        self.service_info = Some(service_info);

        Ok(())
    }

    pub async fn stop(&mut self, forced: bool) {
        // TODO: server context cleanup if needed.

        if let Some(task) = self.mqttc_task.take() {
            if forced {
                info!("Stopping messaging client ...");
                task.abort()
            };
            println!(">>>1111");
            _ = task.await;
            println!(">>>2222");
        };

        info!("Messaging client stopped ...");
        self.mqttc_task = None;
        self.mqttc_client = None;
        self.mqttc_handler = None;
    }

    pub async fn connect(&mut self) -> Result<()> {
        MessagingClient::connect(self).await
    }

    fn password(user: &CryptoIdentity, device: &CryptoIdentity) -> String {
        let nonce = Nonce::random();
        let usr_sig = user.as_ref().sign_into(nonce.as_bytes()).unwrap();
        let dev_sig = device.as_ref().sign_into(nonce.as_bytes()).unwrap();

        let mut pswd = Vec::<u8>::with_capacity(
            nonce.size() + usr_sig.len() + dev_sig.len()
        );

        pswd.extend_from_slice(nonce.as_bytes());
        pswd.extend_from_slice(usr_sig.as_slice());
        pswd.extend_from_slice(dev_sig.as_slice());

        bs58::encode(pswd).into_string()
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
        let sk = channel.session_keypair().private_key().as_bytes();
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

    async fn send_rpc_request<T, R>(&mut self,
        _recipient: &Id,
        _request: RPCRequest<T, R>
    ) -> Result<()>
        where T: Serialize, R: serde::de::DeserializeOwned {

        let msg = MessageBuilder::new(self, MessageType::Call)
            .with_to(_recipient.clone())
            .with_body(serde_cbor::to_vec(&_request).unwrap())
            .build()?;

        self.send_message_intenral(msg).await?;
        Ok(())
    }

    async fn send_message_intenral(&self, msg: Message) -> Result<()> {
        let _type = msg.message_type();
        //let body = msg.body();

        let outbox = self.outbox.as_str();
        let payload = b"TODO".to_vec();
        self.mqttc().publish(
            outbox,
            rumqttc::QoS::AtLeastOnce,
            false,
            payload
        ).await.map_err(|e| {
            Error::State(format!("Failed to send message: {}", e))
        })?;
        unimplemented!()
    }

    async fn attempt_connect(&mut self, urls: Vec<Url>, index: usize) -> Result<()> {
        if self.disconnect {
            return Err(Error::State("Client is stopped".into()));
        }

        let url = urls.get(index).ok_or_else(|| {
            Error::State("No more candidate URLs to connect".into())
        })?;

        let mqtt_options = {
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

        let (mqttc, mut eventloop) = AsyncClient::new(
            mqtt_options,
            10
        );

        let mqttc_handler = {
            let handler = MqttcHandler::new(self);
            Arc::new(Mutex::new(handler))
        };
        self.mqttc_client = Some(mqttc);
        self.mqttc_task = Some(tokio::spawn(async move {
            loop {
                let event = match eventloop.poll().await {
                    Ok(event) => event,
                    Err(e) => {
                        error!("MQTT event loop error: {}, break the loop.", e);
                        break;
                    }
                };

                if let Event::Incoming(packet) = event {
                    mqttc_handler.lock().unwrap().on_mqtt_msg(packet);
                }
            }
        }));
        Ok(())
    }

    async fn do_connect(&mut self) -> Result<()> {
        if let Some(_) = self.mqttc_client.as_ref() {
            if self.is_connected() {
                info!("Already connected to the messaging server");
                return Ok(());
            }
        }

        info!("Connecting to the messaging server ...");
        self.disconnect = false;
        self.user_agent.lock().unwrap().on_connecting();

        let urls = vec![
            Url::parse("tcp://155.138.245.211:1883").unwrap(),
        ];
        self.attempt_connect(urls, 0).await?;

        debug!("Subscribing to the messages ....");
        let topics = vec![
            SubscribeFilter::new(self.inbox.clone(), rumqttc::QoS::AtLeastOnce),
            SubscribeFilter::new(self.outbox.clone(), rumqttc::QoS::AtLeastOnce),
            SubscribeFilter::new("broadcast".to_string(), rumqttc::QoS::AtLeastOnce),
        ];
        self.mqttc().subscribe_many(topics).await.map(|_| {
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
        self.api_client.update_profile(
            name.as_deref(),
            avatar
        ).await
    }

    async fn upload_avatar(
        &mut self,
        content_type: &str,
        avatar: &[u8]
    ) -> Result<String> {
        self.api_client.upload_avatar(content_type, avatar).await
    }

    async fn upload_avatar_from_file(
        &mut self,
        content_type: &str,
        file_name: &str
    ) -> Result<String> {
        self.api_client.upload_avatar_from_file(
            content_type,
            file_name.into()
        ).await
    }

    async fn devices(&mut self) -> Result<Vec<ClientDevice>> {
        _ = RPCRequest::<(), Vec<ClientDevice>>::new(
            self.next_index(),
            RPCMethod::DeviceList,
            None
        );

        // self.send_rpc_request(&self.peer.id(), request).await?;
        unimplemented!()
    }

    async fn revoke_device(
        &mut self,
        device_id: &Id
    ) -> Result<()> {
        let req = RPCRequest::<Id, bool>::new(
            self.next_index(),
            RPCMethod::DeviceRevoke,
            Some(device_id.clone())
        );

        self.send_rpc_request(
            &self.peer.id().clone(),
            req
        ).await
    }

    async fn create_channel(
        &mut self,
        permission: Option<channel::Permission>,
        name: &str,
        notice: Option<&str>
    ) -> Result<Channel> {

        let session_kp = signature::KeyPair::random();
        let session_id = Id::from(session_kp.public_key());
        let permission = permission.unwrap_or(channel::Permission::OwnerInvite);

        let params = parameters::ChannelCreate::new(
            session_id,
            permission,
            Some(name.to_string()),
            notice.map(|n| n.to_string()),
        );

        let mut req = RPCRequest::<parameters::ChannelCreate, bool>::new(
            self.next_index(),
            RPCMethod::ChannelCreate,
            Some(params)
        );

        req.apply_with_cookie(&session_kp, |_| {
            b"cookie".to_vec()
        });

        let peerid = self.service_info.as_ref().unwrap().peerid().clone();
        let rc = self.send_rpc_request(&peerid, req).await;
        if let Err(e) = rc {
            println!("Failed to create channel: {}", e);
            return Err(e);
        }

        // TODO
        unimplemented!()
    }

    async fn remove_channel(
        &mut self,
        channel_id: &Id
    ) -> Result<()> {
        let req = RPCRequest::<(), bool>::new(
            self.next_index(),
            RPCMethod::ChannelRemove,
            None
        );
        self.send_rpc_request(channel_id, req).await
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

        let mut req = RPCRequest::<InviteTicket, Channel>::new(
            self.next_index(),
            RPCMethod::ChannelJoin,
            Some(ticket.proof().clone())
        );
        req.apply_with_cookie(session_key, |_| {
            Vec::<u8>::new() // TODO
        });
        self.send_rpc_request(ticket.channel_id(), req).await
    }

    async fn leave_channel(&mut self, channel_id: &Id) -> Result<()> {
        let request = RPCRequest::<(), bool>::new(
            self.next_index(),
            RPCMethod::ChannelLeave,
            None
        );
        self.send_rpc_request(channel_id, request).await
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
        let ua = self.user_agent.lock().unwrap();
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

        let req = RPCRequest::<Id, bool>::new(
            self.next_index(),
            RPCMethod::ChannelOwner,
            Some(new_owner.clone())
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_permission(
        &mut self,
        channel_id: &Id,
        permission: Permission
    ) -> Result<()> {
        let ua = self.user_agent.lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };

        if !channel.is_owner(self.user.id()) {
            Err(Error::State("Not channel owner".into()))?
        }
        drop(ua);

        let req = RPCRequest::<Permission, bool>::new(
            self.next_index(),
            RPCMethod::ChannelPermission,
            Some(permission)
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_name(
        &mut self,
        channel_id: &Id,
        name: Option<&str>
    ) -> Result<()> {
        let ua = self.user_agent.lock().unwrap();
        let Some(channel) = ua.channel(channel_id)? else {
            Err(Error::State("Channel does not exist".into()))?
        };

        let userid = self.user.id();
        if !channel.is_owner(userid) && !channel.is_moderator(userid) {
            Err(Error::State("Not channel owner or moderator".into()))?
        }
        drop(ua);

        let name = name.map(|n|
            match n.is_empty() {
                true => None,
                false => Some(n.nfc().collect::<String>())
            }
        ).flatten();
        let req = RPCRequest::<String, bool>::new(
            self.next_index(),
            RPCMethod::ChannelName,
            name
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_notice(
        &mut self,
        channel_id: &Id,
        notice: Option<&str>
    ) -> Result<()> {
        let ua = self.user_agent.lock().unwrap();
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

        let req = RPCRequest::<String, bool>::new(
            self.next_index(),
            RPCMethod::ChannelNotice,
            notice
        );
        self.send_rpc_request(channel_id, req).await
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

        let ua = self.user_agent.lock().unwrap();
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
        let req = RPCRequest::<parameters::ChannelMemberRole, bool>::new(
            self.next_index(),
            RPCMethod::ChannelRole,
            Some(role)
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn ban_channel_members(
        &mut self,
        channel_id: &Id,
        members: Vec<&Id>,
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(())
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
        let req = RPCRequest::<Vec<Id>, bool>::new(
            self.next_index(),
            RPCMethod::ChannelBan,
            Some(members)
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn unban_channel_members(
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
        let req = RPCRequest::<Vec<Id>, bool>::new(
            self.next_index(),
            RPCMethod::ChannelUnban,
            Some(members)
        );
        self.send_rpc_request(channel_id, req).await
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
        let req = RPCRequest::<Vec<Id>, bool>::new(
            self.next_index(),
            RPCMethod::ChannelRemove,
            Some(members)
        );
        self.send_rpc_request(channel_id, req).await
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
        let mut ua = self.user_agent.lock().unwrap();
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

struct MqttcHandler {
    user_agent      : Arc<Mutex<dyn UserAgent>>,
    pending_msgs    : Arc<Mutex<HashMap<u16, Message>>>,
    connect_promise : Option<Box<dyn FnMut() + Send + Sync>>,

    server_context  : Option<CryptoContext>,    // TODO,
    disconnect      : bool,
    failures        : u32,


    peer            : PeerInfo,

    inbox           : String,
    outbox          : String,
    broadcast       : String,
}

impl MqttcHandler {
    #[allow(unused)]
    fn new(client: &Client) -> Self {
        Self {
            user_agent      : client.ua(),
            pending_msgs    : client.pending_msgs.clone(),
            connect_promise : None,
            server_context  : None,
            disconnect      : false,
            failures        : 0,
            peer            : client.peer.clone(),

            inbox           : client.inbox.clone(),
            outbox          : client.outbox.clone(),
            broadcast       : "broadcast".into(),
        }
    }

    fn is_me(&self, _id: &Id) -> bool {
        true // TODO
    }

    fn on_mqtt_msg(&mut self, packet: Packet) {
        match packet {
            Packet::Publish(publish) => self.on_message(publish),
            Packet::PubAck(ack) => self.on_publish_comletion(ack),
            Packet::SubAck(ack) => self.on_subscribe_completion(ack),
            Packet::UnsubAck(ack) => self.on_unsubscribe_completion(ack),
            Packet::Disconnect => self.on_close(),
            Packet::PingResp => self.on_ping_response(),

            _ => info!("Unknown MQTT event: {:?}", packet)
        }
    }

    fn on_publish_comletion(&mut self, ack: rumqttc::PubAck) {
        match self.pending_msgs.lock().unwrap().remove(&ack.pkid) {
            Some(msg) => msg.on_sent(),
            None => {
                error!("INTERNAL ERROR: no message associated with packet {}, skipped pub acked event", ack.pkid);
            }
        }
    }

    fn on_subscribe_completion(&mut self, ack: rumqttc::SubAck) {
        for rc in ack.return_codes {
            match rc {
                rumqttc::SubscribeReasonCode::Success(qos) =>
                    info!("Subscription acknowledged with QoS: {:?}", qos),
                rumqttc::SubscribeReasonCode::Failure =>
                    error!("Subscription failed with error.")
            }
        }

        if let Some(connect_promise) = self.connect_promise.as_mut() {
            info!("Subscribe topics success");
            self.user_agent.lock().unwrap().on_connected();
            connect_promise();
        }
    }

    fn on_unsubscribe_completion(&mut self, _: rumqttc::UnsubAck) {}

    fn on_close(&mut self) {
        if self.disconnect {
            self.user_agent.lock().unwrap().on_disconnected();
            info!("disconnected!");
            return;
        }

        self.failures += 1;
        error!("Connection lost, attempt to reconnect in {} seconds...", 5); // TODO:

    }

    fn on_ping_response(&mut self) {
        info!("Ping response received");
    }

    fn on_message(&mut self, publish: rumqttc::Publish) {
        let topic = publish.topic.as_str();
        debug!("Got message on topic: {}", topic);

        let payload = self.server_context.as_mut().unwrap().decrypt_into(
            publish.payload.as_ref()
        ).unwrap_or_else(|_| {
            error!("Failed to decrypt message payload");
            Vec::new()
        });

        let Ok(mut msg) = serde_cbor::from_slice::<Message>(&payload) else {
            error!("Failed to deserialize message from {topic}, ignored");
            return;
        };

        if msg.is_valid() {
            error!("Invalid message received: {topic}, ignored");
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
            error!("Unknown topic: {topic}, ignored");
            return;
        }
    }

    fn on_inbox_message(&mut self, msg: Message) {
        let Ok(mtype) = msg.message_type() else {
            return;
        };

        if let Some(v) = msg.body().as_ref() {
            if v.len() > 0 && msg.from() == self.peer.id() {
                if self.is_me(&msg.to()) {
                    info!("Received message from myself, ignored: {:?}", msg);

                    match mtype {
                        MessageType::Message => {
                            let sender = self.user_agent.lock().unwrap().contact(msg.from());
                            if let Ok(Some(sender)) = sender {
                                if sender.has_session_key() {
                                    //msg.decrypt_body(sender.rx_crypto_context());
                                    info!("Decrypted message from self: {:?}", msg);
                                } else {
                                    error!("Sender contact does not have session key: {:?}", msg);
                                }
                            } else {
                                error!("Failed to get sender contact for message: {:?}", msg);
                            }
                        },
                        MessageType::Call => {}, // TODO: handle call
                        MessageType::Notification => {}, // TODO: handle notification
                    }
                    return;
                } else { // received message for channel.
                    // Message: sender -> channel
                }
            }
        };
        unimplemented!()
    }

    fn on_outbox_message(&mut self, _message: Message) {
        unimplemented!()
    }

    fn on_broadcast_message(&mut self, _message: Message) {
        unimplemented!()
    }
}
