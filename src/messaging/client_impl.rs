use std::time::Duration;
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;
use serde::Serialize;
use sha2::{Digest, Sha256};
use url::Url;
use log::{debug, info, error};
use tokio::task::JoinHandle;
use rumqttc::{
    MqttOptions,
    AsyncClient,
    SubscribeFilter,
    Event,
    Packet
};
use md5;

use crate::{
    Id,
    Identity,
    PeerInfo,
    cryptobox::Nonce,
    core::{
        Error,
        Result,
        CryptoIdentity,
        CryptoContext
    },
    messaging::{
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

    service_info    : Option<api_client::MessagingServiceInfo>,

    server_context  : CryptoContext,
    self_context    : CryptoContext,

    api_client      : APIClient,
    disconnect      : bool,

    mqtt_client     : Option<AsyncClient>,
    task_handler    : Option<JoinHandle<()>>,

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
            service_info: None,

            client_id,
            inbox       : format!("inbox/{}", bs58_userid),
            outbox      : format!("outbox/{}", bs58_userid),

            user_agent  : user_agent.clone(),
            disconnect  : false,
            api_client,

            mqtt_client : None,
            task_handler: None,

            self_context,
            server_context,
        })
    }

    fn mqttc(&mut self) -> &AsyncClient {
        self.mqtt_client
            .as_ref()
            .expect("MQTT Async client should be created")
    }

    fn next_index(&mut self) -> u32 { 0 }

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

        self.service_info = Some(self.api_client.service_info().await?);
        Ok(())
    }

    pub async fn stop(&mut self, forced: bool) {
        // TODO: server context cleanup if needed.

        info!("Stopping messaging client ...");
        if let Some(handle) = self.task_handler.take() {
            if forced {
                handle.abort()
            };
            handle.await.ok();
        };

        info!("Messaging client stopped ...");
        self.task_handler = None;
        self.mqtt_client = None;
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

    async fn send_rpc_request<T, R>(&mut self, _channel_id: &Id, _request: RPCRequest<T, R>) -> Result<()>
    where T: Serialize, R: serde::de::DeserializeOwned {
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

        let (mqtt_client, mut eventloop) = AsyncClient::new(
            mqtt_options,
            10
        );

        self.mqtt_client = Some(mqtt_client);
        self.task_handler = Some(tokio::spawn(async move {
            loop {
                let result = eventloop.poll().await;
                if let Err(e) = result {
                    error!("MQTT event loop error: {}", e);
                    break;
                }
                let event = result.unwrap();
                println!("Received = {:?}", event);
                match event {
                    Event::Incoming(event) => {
                        match event {
                            Packet::SubAck(ack) => {
                                info!("Subscription acknowledged: {:?}", ack);
                            },
                            Packet::UnsubAck(ack) => {
                                info!("Unsubscription acknowledged: {:?}", ack);
                            },
                            Packet::Disconnect => {
                                info!("Disconnected from the MQTT broker");
                                break;
                            },
                            Packet::PingResp => {
                                info!("Ping response received");
                            },
                            _ => {
                                info!("Unknown Mqtt event: {:?}", event);
                            }
                        }
                    },
                    _ => {}
                }
            }
        }));
        Ok(())
    }

    async fn do_connect(&mut self) -> Result<()> {
        if let Some(_) = self.mqtt_client.as_ref() {
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

    async fn create_invite_ticket(&mut self,
        channel_id: &Id
    ) -> Result<InviteTicket> {
        self.sign_into_invite_ticket(channel_id, None).await
    }

    async fn create_invite_ticket_with_invitee(&mut self,
        channel_id: &Id,
        invitee: &Id
    ) -> Result<InviteTicket> {
        self.sign_into_invite_ticket(channel_id, Some(invitee)).await
    }

    async fn set_channel_owner(&mut self,
        channel_id: &Id,
        new_owner: &Id
    ) -> Result<()> {
        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) {
            return Err(Error::State("Not channel owner".into()));
        }

        if channel.is_member(new_owner) {
            return Err(Error::State("New owner is not in the channel".into()));
        }
        drop(locked_agent);

        let req = RPCRequest::<Id, bool>::new(
            self.next_index(),
            RPCMethod::ChannelOwner,
            new_owner.clone()
        );

        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_permission(&mut self,
        channel_id: &Id,
        permission: Permission
    ) -> Result<()> {
        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) {
            return Err(Error::State("Not channel owner".into()));
        }
        drop(locked_agent);

        let req = RPCRequest::<Permission, bool>::new(
            self.next_index(),
            RPCMethod::ChannelPermission,
            permission
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_name(&mut self,
        channel_id: &Id,
        name: Option<&str>
    ) -> Result<()> {
        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }
        drop(locked_agent);

        let name = name.map(|n| n.nfc().collect::<String>())
            .unwrap_or_default();

        let req = RPCRequest::<String, bool>::new(
            self.next_index(),
            RPCMethod::ChannelName,
            name
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_notice(&mut self,
        channel_id: &Id,
        notice: Option<&str>
    ) -> Result<()> {

        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }
        drop(locked_agent);

        let notice = notice.map(|n| n.nfc().collect::<String>())
            .unwrap_or_default();

        let req = RPCRequest::<String, bool>::new(
            self.next_index(),
            RPCMethod::ChannelNotice,
            notice
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn set_channel_member_role(&mut self,
        channel_id: &Id,
        members: Vec<&Id>,
        role: Role
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }
        drop(locked_agent);

        let req = RPCRequest::<Role, bool>::new(
            self.next_index(),
            RPCMethod::ChannelRole,
            role
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn ban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<Id>,
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }
        drop(locked_agent);

        let req = RPCRequest::<Vec<Id>, bool>::new(
            self.next_index(),
            RPCMethod::ChannelBan,
            members
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn unban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<Id>
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }
        drop(locked_agent);

        let req = RPCRequest::<Vec<Id>, bool>::new(
            self.next_index(),
            RPCMethod::ChannelUnban,
            members
        );
        self.send_rpc_request(channel_id, req).await
    }

    async fn remove_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<Id>
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }

        let locked_agent = self.user_agent.lock().unwrap();
        let Some(channel) = locked_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }
        drop(locked_agent);

        let req = RPCRequest::<Vec<Id>, bool>::new(
            self.next_index(),
            RPCMethod::ChannelRemove,
            members
        );
        self.send_rpc_request(channel_id, req).await
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
