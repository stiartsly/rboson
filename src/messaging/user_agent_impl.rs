use std::path::Path;
use std::collections::LinkedList;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use log::{error};

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

use super::{
    conversation::Conversation,
    message::Message,
    channel::{Member, Channel, Role},
    messaging_repository::MessagingRepository,
    persistence::database::Database,
    contact::Contact,

    user_profile::UserProfile,
    device_profile::DeviceProfile,
    connection_listener::ConnectionListener,
    profile_listener::ProfileListener,
    message_listener::MessageListener,
    channel_listener::ChannelListener,
    contact_listener::ContactListener,
};

#[allow(dead_code)]
pub struct DefaultUserAgent {
    user    : Option<UserProfile>,
    device  : Option<DeviceProfile>,
    peer    : Option<PeerInfo>,

    repository  : Option<Database>,

    connection_listeners: LinkedList<Box<dyn ConnectionListener>>,
    profile_listeners   : LinkedList<Box<dyn ProfileListener>>,
    message_listeners   : LinkedList<Box<dyn MessageListener>>,
    channel_listeners   : LinkedList<Box<dyn ChannelListener>>,
    contact_listeners   : LinkedList<Box<dyn ContactListener>>,

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

            connection_listeners: LinkedList::new(),
            profile_listeners   : LinkedList::new(),
            message_listeners   : LinkedList::new(),
            channel_listeners   : LinkedList::new(),
            contact_listeners   : LinkedList::new(),

            conversations: HashMap::new(),

            hardened: false,
        })
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

        self.device = Some(DeviceProfile::new(Some(device), name, app));
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
            #[serde(with = "super::bytes_as_base64")]
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
            #[serde(with = "super::bytes_as_base64")]
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
            #[serde(with = "super::bytes_as_base64")]
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
            #[serde(with = "super::bytes_as_base64")]
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
            Some(CryptoIdentity::from_private_key(&sk)),
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
            #[serde(with = "super::bytes_as_base64")]
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

impl MessageListener for DefaultUserAgent {
    fn on_message(&self, _message: &Message) {
        unimplemented!()
    }

    fn on_sending(&self, _message: &Message) {
        unimplemented!()
    }

    fn on_sent(&self, _message: &Message) {
        unimplemented!()
    }

    fn on_broadcast(&self, _message: &Message) {
        unimplemented!()
    }
}

impl ProfileListener for DefaultUserAgent {
    fn on_user_profile_acquired(&self, _profile: &UserProfile) {
        unimplemented!()
    }

    fn on_user_profile_changed(&self, _avatar: bool) {
        unimplemented!()
    }
}

impl ConnectionListener for DefaultUserAgent {
    fn on_connection(&self) {
        unimplemented!()
    }

    fn on_connected(&self) {
        unimplemented!()
    }

    fn on_disconnected(&self) {
        unimplemented!()
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

    fn channels(&self) -> Vec<Channel> {
        unimplemented!()
    }

    fn channel(&self, _channel_id: &Id) -> Option<Channel> {
        unimplemented!()
    }

    fn contact_version(&self) -> String {
        unimplemented!()
    }


}