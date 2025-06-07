use std::path::Path;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use log::{error, warn};

use crate::{
    unwrap,
    Id,
    PeerInfo,
    error::Result,
    Error,
    signature::PrivateKey,
    core::crypto_identity::CryptoIdentity,
    messaging::user_agent::UserAgent,
};

use crate::messaging::{
    Contact,
    Conversation,
    UserProfile,
    DeviceProfile,
    ProfileListener,
    ContactListener,
    ConnectionListener,
    MessageListener,
};

use super::{
    message::Message,
    channel::{Member, Channel, Role},
    messaging_repository::MessagingRepository,
    persistence::database::Database,

    profile_listener::ProfileListenerMut,
    message_listener::MessageListenerMut,
    channel_listener::ChannelListener,
};

#[allow(dead_code)]
pub struct DefaultUserAgent {
    user    : Option<UserProfile>,
    device  : Option<DeviceProfile>,
    peer    : Option<PeerInfo>,

    repository  : Option<Database>,

    connection_listener : Option<Box<dyn ConnectionListener>>,
    profile_listener    : Option<Box<dyn ProfileListener>>,
    message_listener    : Option<Box<dyn MessageListener>>,
    channel_listener    : Option<Box<dyn ChannelListener>>,
    contact_listener    : Option<Box<dyn ContactListener>>,

    conversations: HashMap<Id, Conversation>,

    hardened: bool,
}

#[allow(unused)]
impl DefaultUserAgent {
    pub fn new(_path: Option<&Path>) -> Result<Self> {
        Ok(Self {
            user    : None,
            device  : None,
            peer    : None,

            repository: None,

            connection_listener : None,
            profile_listener    : None,
            message_listener    : None,
            channel_listener    : None,
            contact_listener    : None,

            conversations: HashMap::new(),

            hardened: false,
        })
    }

    fn is_myself(&self, id: &Id) -> bool {
        self.user.as_ref().unwrap().id() == id
    }

    pub(crate) fn harden(&mut self) {
        self.hardened = true;
    }

    pub(crate) fn set_user(&mut self, user: CryptoIdentity, name: String) -> Result<()>{
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.user = Some(UserProfile::new(user, name, false));
        self.update_userinfo_config();
        Ok(())
    }

    pub(crate) fn set_device(&mut self, device: CryptoIdentity, name: String, app: Option<String>) -> Result<()> {
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.device = Some(DeviceProfile::new(device, name, app));
        self.update_device_info_config();
        Ok(())
    }

    pub(crate) fn set_messaging_peer_info(&mut self, peer: &PeerInfo) -> Result<()> {
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        if !peer.is_valid() {
            return Err(Error::Argument("Peer info {peer} is invalid!".into()));
        }
        if peer.alternative_url().is_none() {
            return Err(Error::Argument("Peer url must be set".into()));
        }

        self.peer = Some(peer.clone());
        self.udpate_messaging_peerinfo();
        Ok(())
    }

    pub(crate) fn set_repository(&mut self, repository: Database) -> Result<()> {
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.repository = Some(repository);
        self.load_config()?;
        self.conversations.clear();
        //self.repository.all_conversations().for_each(|c| {
        //    self.conversations.insert(c.id().clone(), c);
        //});
        Ok(())
    }

    fn update_userinfo_config(&mut self) {
        let Some(repo) = self.repository.as_ref() else {
            error!("Repository is not configured!");
            return;
        };

        #[derive(Serialize, Debug)]
        #[allow(non_snake_case)]
        struct UserInfo<'a> {
            #[serde(with = "super::serde_bytes_with_base64")]
            privateKey  : &'a [u8],
            name        : &'a str,
            #[serde(skip)]
            avatar      : bool
        }

        let user = UserInfo {
            privateKey: unwrap!(self.user).identity().keypair().private_key().as_bytes(),
            name: unwrap!(self.user).name(),
            avatar: unwrap!(self.user).has_avatar()
        };

        self.repository.as_ref().map(|v| {
            if let Err(e) = v.put_config_mult(".user", &user) {
                error!("Save user profile failed, error: {e}");
            }
        });
    }

    fn update_device_info_config(&self) {
        let Some(repo) = self.repository.as_ref() else {
            error!("Repository is not configured!");
            return;
        };

        #[derive(Serialize, Debug)]
        #[allow(non_snake_case)]
        struct DeviceInfo<'a> {
            #[serde(with = "super::serde_bytes_with_base64")]
            privateKey  : &'a [u8],
            name        : &'a str,
            app         : Option<&'a str>
        }

        let user = DeviceInfo {
            privateKey: unwrap!(unwrap!(self.device).identity()).keypair().private_key().as_bytes(),
            name: unwrap!(self.device).name(),
            app: unwrap!(self.device).app()
        };

        self.repository.as_ref().map(|v| {
            if let Err(e) = v.put_config_mult(".device", &user) {
                error!("Save device profile failed, error: {e}");
            }
        });
    }

    fn udpate_messaging_peerinfo(&self) {
        let Some(repo) = self.repository.as_ref() else {
            error!("Repository is not configured!");
            return;
        };

        #[derive(Serialize, Debug)]
        #[allow(non_snake_case)]
        struct PeerInfo<'a> {
            peerId: &'a [u8],
            nodeId: &'a [u8],
            apiUrl: Option<&'a str>
        }

        let user = PeerInfo {
            peerId: unwrap!(self.peer).id().as_bytes(),
            nodeId: unwrap!(self.peer).origin().as_bytes(),
            apiUrl: unwrap!(self.peer).alternative_url()
        };

        self.repository.as_ref().map(|v| {
            if let Err(e) = v.put_config_mult(".peer", &user) {
                error!("Save messaging peer info failed, error: {e}");
            }
        });
    }

    fn load_config(&mut self) -> Result<()> {
        let Some(repo) = self.repository.as_ref() else {
            return Err(Error::State("Messaging repository is not configured!".into()));
        };

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct UserInfo {
            #[serde(with = "super::serde_bytes_with_base64")]
            privateKey  : Vec<u8>,
            name        : String,
            #[serde(skip)]
            avatar      : bool
        }

        let user = repo.get_config_mult::<UserInfo>(".user").map_err(|e|
            Error::State("Load user profile failed, error {e}".into())
        )?;
        let sk = PrivateKey::try_from(user.privateKey.as_slice()).map_err(|e|
            Error::State("Invalid private key: {e}".into())
        )?;

        let user = UserProfile::new(
            CryptoIdentity::from_private_key(&sk),
            user.name,
            user.avatar
        );

        #[derive(Serialize, Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct DeviceInfo {
            #[serde(with = "super::serde_bytes_with_base64")]
            privateKey  : Vec<u8>,
            name        : String,
            app         : Option<String>
        }

        let device = repo.get_config_mult::<DeviceInfo>(".device").map_err(|e|
            Error::State("Load device profile failed, error {e}".into())
        )?;
        let sk = PrivateKey::try_from(device.privateKey.as_slice()).map_err(|e|
            Error::State("Invalid private key: {e}".into())
        )?;

        let device = DeviceProfile::new(
            CryptoIdentity::from_private_key(&sk),
            device.name,
            device.app
        );

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct PeerInfo {
            peerId: Id,
            nodeId: Id,
            port: u16,
            apiUrl: Option<String>,
            #[serde(with = "super::serde_bytes_with_base64")]
            sig: Vec<u8>
        }

        let peer = repo.get_config_mult::<PeerInfo>(".peer")
            .map_err(|e|
                Error::State("Load messaging peer info failed, error {e}".into())
            )?;

        // TODO: compose PeerInfo.

        self.user = Some(user);
        self.device = Some(device);
        self.peer = None;
        Ok(())
    }

    fn put_message(&mut self, message: Message) {
        unimplemented!()
    }
}

impl ContactListener for DefaultUserAgent {
    fn on_contacts_updating(&self, _version_id: &str, _contacts: Vec<Contact>) {}
    fn on_contacts_updated(&self, _base_version_id: &str, _new_version_id: &str, _contacts: Vec<Contact>) {}
    fn on_contacts_cleared(&self) {}
    fn on_contact_profile(&self, _contact_id: &Id, _profile: &Contact) {}
}

impl ChannelListener for DefaultUserAgent {
    fn on_joined_channel(&self, _channel: &Channel) {
        unimplemented!()
    }

    fn on_left_channel(&self, _channel: &Channel) {
        unimplemented!()
    }

    fn on_channel_deleted(&self, _channel: &Channel) {
        unimplemented!()
    }

    fn on_channel_updated(&self, _channel: &Channel) {
        unimplemented!()
    }

    fn on_channel_members(&self, _channel: &Channel, _members: &[Member]) {
        unimplemented!()
    }

    fn on_channel_member_joined(&self, _channel: &Channel, _member: &Member) {
        unimplemented!()
    }

    fn on_channel_member_left(&self, _channel: &Channel, _member: &Member) {
        unimplemented!()
    }

    fn on_channel_members_removed(&self, _channel: &Channel, _members: &[Member]) {
        unimplemented!()
    }

    fn on_channel_members_banned(&self, _channel: &Channel, _anned: &[Member]) {
        unimplemented!()
    }

    fn on_channel_members_unbanned(&self, _channel: &Channel, _unbanned: &[Member]) {
        unimplemented!()
    }

    fn on_channel_members_role_changed(&self,
        _channel: &Channel,
        _changed: &[Member],
        _role: Role,
    ) {
        unimplemented!()
    }
}

impl MessageListenerMut for DefaultUserAgent {
    fn on_message(&mut self, mut message: Message) {
        let is_channel_message = !self.is_myself(message.to());
        let conv_id = match is_channel_message {
            true => message.to(),
            false => message.from(),
        }.clone();

        message.set_conversation_id(&conv_id);

        self.message_listener.as_mut().map(|l| {
            l.on_message(&message);
        });

        self.repository.as_mut().map(|v| {
            if let Err(e) = v.put_message(message) {
                error!("Save message failed, error: {e}");
            }
        });
    }

    fn on_sending(&mut self, _message: Message) {
        unimplemented!()
    }

    fn on_sent(&mut self, _message: Message) {
        unimplemented!()
    }

    fn on_broadcast(&mut self, _message: Message) {
        unimplemented!()
    }
}

impl ProfileListenerMut for DefaultUserAgent {
    fn on_user_profile_acquired(&mut self, profile: UserProfile) {
        if let Some(ref user) = self.user {
            if user.id() != profile.id() {
                warn!("User profile acquired with different id: {} != {}", user.id(), profile.id());
            }
        }

        self.user = Some(profile);
        self.update_userinfo_config();
        self.profile_listener.as_mut().map(|l| {
            l.on_user_profile_acquired(self.user.as_ref().unwrap());
        });
    }

    fn on_user_profile_changed(&mut self, name: String, avatar: bool) {
        let Some(ref user) = self.user else {
            warn!("User profile is not set!");
            return;
        };

        self.user = Some(UserProfile::new(
            user.identity().clone(),
            name,
            avatar
        ));

        self.update_userinfo_config();
        self.profile_listener.as_mut().map(|l| {
            l.on_user_profile_changed(avatar);
        });
    }
}

impl ConnectionListener for DefaultUserAgent {
    fn on_connecting(&self) {
        self.connection_listener.as_ref().map(|l| {
            l.on_connecting();
        });
    }

    fn on_connected(&self) {
        self.connection_listener.as_ref().map(|l| {
            l.on_connected();
        });
    }

    fn on_disconnected(&self) {
        self.connection_listener.as_ref().map(|l| {
            l.on_disconnected();
        });
    }
}

impl UserAgent for DefaultUserAgent {
    fn user(&self) -> Option<&UserProfile> {
        self.user.as_ref()
    }

    fn device(&self) -> Option<&DeviceProfile> {
        self.device.as_ref()
    }

    fn peer_info(&self) -> Option<&PeerInfo> {
        self.peer.as_ref()
    }

    fn is_configured(&self) -> bool {
        self.user.is_some() &&
            self.device.is_some() &&
            self.peer.is_some() &&
            self.repository.is_some() &&
            self.peer.as_ref().unwrap().is_valid() &&
            self.peer.as_ref().unwrap().alternative_url().is_some()
    }

    fn set_connection_listener(&mut self, listener: Box<dyn ConnectionListener>) {
        self.connection_listener = Some(listener);
    }

    fn set_profile_listener(&mut self, listener: Box<dyn ProfileListener>) {
        self.profile_listener = Some(listener);
    }

    fn set_message_listener(&mut self, listener: Box<dyn MessageListener>) {
        self.message_listener = Some(listener);
    }

    fn set_channel_listener(&mut self, listener: Box<dyn ChannelListener>) {
        self.channel_listener = Some(listener);
    }

    fn set_contact_listener(&mut self, listener: Box<dyn ContactListener>) {
        self.contact_listener = Some(listener);
    }

    fn conversation(&self, _conversation_id: &Id) -> Option<Conversation> {
        unimplemented!()
    }

    fn conversations(&self) -> Vec<Conversation> {
        unimplemented!()
    }

    fn remove_conversation(&mut self, _converstation_id: &Id) {
        unimplemented!()
    }

    fn remove_conversations(&mut self, _converstation_ids: Vec<&Id>) {
        unimplemented!()
    }

    fn messages(&self, converstation_id: &Id) -> Vec<Message> {
        self.repository.as_ref().map(|v| {
            v.messages_since(converstation_id, 0, 100, 0)
        })
        .unwrap_or_else(|| Ok(vec![]))
        .unwrap()
    }

    fn messages_between(&self, converstation_id: &Id, from: u64, end: u64) -> Vec<Message> {
        self.repository.as_ref().map(|v| {
            v.messages_between(converstation_id, from, end)
        })
        .unwrap_or_else(|| Ok(vec![]))
        .unwrap()
    }

    fn messages_since(&self, converstation_id: &Id, since: u64, limit: usize, offset: usize) -> Vec<Message> {
        self.repository.as_ref().map(|v| {
            v.messages_since(converstation_id, since, limit, offset)
        })
        .unwrap_or_else(|| Ok(vec![]))
        .unwrap()
    }

    fn remove_message(&mut self, messsage_id: u32) {
        self.repository.as_mut().map(|v| {
            _ = v.remove_amessage(messsage_id);
        });
    }

    fn remove_messages(&mut self, message_ids: &[u32]) {
        self.repository.as_mut().map(|v| {
            _ = v.remove_messages(message_ids);
        });
    }

    fn remove_messages_by_conversation(&mut self, converstation_id: &Id) {
        self.repository.as_mut().map(|v| {
            _ = v.remove_messages_by_conversation(converstation_id);
        });
    }

    fn channels(&self) -> Result<Vec<Channel>> {
        unimplemented!()
    }

    fn channel(&self, _channel_id: &Id) -> Result<Option<Channel>> {
        unimplemented!()
    }

    fn contact_version(&self) -> Result<Option<String>> {
        unimplemented!()
    }

    fn put_contacts_update(&mut self,
        _version_id: &str,
        _contacts: &[Contact]
    ) -> Result<()> {
        unimplemented!()
    }
}
