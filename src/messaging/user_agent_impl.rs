use std::path::Path;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use log::{error, warn};

use crate::{
    unwrap,
    Id,
    PeerInfo,
    core::{
        Error,
        Result,
        CryptoIdentity
    }
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
    user_agent_caps::UserAgentCaps,

    message::Message,
    contact::GenericContact,
    channel::{self, Member, Channel, ChannelData, Role},
    messaging_repository::MessagingRepository,
    persistence::database::Database,

    profile_listener::ProfileListenerMut,
    message_listener::MessageListenerMut,
    channel_listener::ChannelListener,

};

#[allow(dead_code)]
pub struct UserAgent {
    user        : Option<UserProfile>,
    device      : Option<DeviceProfile>,
    peer        : Option<PeerInfo>,
    repo        : Option<Database>,

    connection_listeners: Vec<Box<dyn ConnectionListener>>,
    profile_listeners   : Vec<Box<dyn ProfileListener>>,
    message_listeners   : Vec<Box<dyn MessageListener>>,
    channel_listeners   : Vec<Box<dyn ChannelListener>>,
    contact_listeners   : Vec<Box<dyn ContactListener>>,

    conversations       : HashMap<Id, Conversation>,

    hardened: bool,
}

#[allow(unused)]
impl UserAgent {
    pub fn new(_path: Option<&Path>) -> Result<Self> {
        Ok(Self {
            user                : None,
            device              : None,
            peer                : None,
            repo                : None,
            connection_listeners: Vec::new(),
            profile_listeners   : Vec::new(),
            message_listeners   : Vec::new(),
            channel_listeners   : Vec::new(),
            contact_listeners   : Vec::new(),
            conversations       : HashMap::new(),

            hardened: false,
        })
    }

    fn is_myself(&self, id: &Id) -> bool {
        self.user.as_ref().map(|v| v.id() == id).unwrap_or(false)
    }

    pub(crate) fn harden(&mut self) {
        self.hardened = true;
    }

    pub fn set_user(&mut self, user: CryptoIdentity, name: Option<&str>) -> Result<()>{
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.user = Some(UserProfile::new(user, name.map(|v| v.into()).unwrap(), false));
        self.update_userinfo_config();
        Ok(())
    }

    pub fn set_device(&mut self,
        device: CryptoIdentity,
        name: Option<&str>,
        app_name: Option<&str>
    ) -> Result<()> {
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.device = Some(DeviceProfile::new(
            Some(device),
            name.map(|v|v.into()),
            app_name.map(|v|v.into())
        ));
        self.update_device_info_config();
        Ok(())
    }

    pub fn set_messaging_peer_info(&mut self, peer: &PeerInfo) -> Result<()> {
        if self.hardened {
            Err(Error::State("UserAgent is hardened".into()))?;
        }
        if !peer.is_valid() {
            Err(Error::Argument("Peer info {peer} is invalid!".into()))?;
        }

        self.peer = Some(peer.clone());
        self.udpate_messaging_peerinfo();
        Ok(())
    }

    pub(crate) fn set_repository(&mut self, repository: Database) -> Result<()> {
        if self.hardened {
            return Err(Error::State("UserAgent is hardened".into()));
        }

        self.repo = Some(repository);
        self.load_config()?;
        self.conversations.clear();
        //self.repository.all_conversations().for_each(|c| {
        //    self.conversations.insert(c.id().clone(), c);
        //});
        Ok(())
    }

    fn update_userinfo_config(&mut self) {
        let Some(repo) = self.repo.as_ref() else {
            error!("Repository is not configured!");
            return;
        };

        #[derive(Serialize, Debug)]
        #[allow(non_snake_case)]
        struct UserInfo<'a> {
            #[serde(with = "crate::serde_bytes_base64")]
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

        self.repo.as_ref().map(|v| {
            if let Err(e) = v.put_config_mult(".user", &user) {
                error!("Save user profile failed, error: {e}");
            }
        });
    }

    fn update_device_info_config(&self) {
        let Some(repo) = self.repo.as_ref() else {
            error!("Repository is not configured!");
            return;
        };

        #[derive(Serialize, Debug)]
        #[allow(non_snake_case)]
        struct DeviceInfo<'a> {
            #[serde(with = "crate::serde_bytes_base64")]
            privateKey  : &'a [u8],
            name        : &'a str,
            app         : Option<&'a str>
        }

        let user = DeviceInfo {
            privateKey: unwrap!(unwrap!(self.device).identity()).keypair().private_key().as_bytes(),
            name: unwrap!(self.device).name().unwrap(),
            app: unwrap!(self.device).app_name()
        };

        self.repo.as_ref().map(|v| {
            if let Err(e) = v.put_config_mult(".device", &user) {
                error!("Save device profile failed, error: {e}");
            }
        });
    }

    fn udpate_messaging_peerinfo(&self) {
        let Some(repo) = self.repo.as_ref() else {
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

        self.repo.as_ref().map(|v| {
            if let Err(e) = v.put_config_mult(".peer", &user) {
                error!("Save messaging peer info failed, error: {e}");
            }
        });
    }

    fn load_config(&mut self) -> Result<()> {
        let Some(repo) = self.repo.as_ref() else {
            return Err(Error::State("Messaging repository is not configured!".into()));
        };

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct UserInfo {
            #[serde(with = "crate::serde_bytes_base64")]
            privateKey  : Vec<u8>,
            name        : String,
            #[serde(skip)]
            avatar      : bool
        }

        let user = repo.get_config_mult::<UserInfo>(".user").map_err(|e|
            Error::State("Load user profile failed, error {e}".into())
        )?;
        let identity = CryptoIdentity::from_private_key(user.privateKey.as_slice()).map_err(|e| {
            Error::State(format!("Failed to create CryptoIdentity from private key: {e}"))
        })?;

        let user = UserProfile::new(
            identity,
            user.name,
            user.avatar
        );

        #[derive(Serialize, Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct DeviceInfo {
            #[serde(with = "crate::serde_bytes_base64")]
            privateKey  : Vec<u8>,
            name        : String,
            app         : Option<String>
        }

        let device = repo.get_config_mult::<DeviceInfo>(".device").map_err(|e|
            Error::State("Load device profile failed, error {e}".into())
        )?;
        let identity = CryptoIdentity::from_private_key(device.privateKey.as_slice()).map_err(|e| {
            Error::State(format!("Failed to create CryptoIdentity from private key: {e}"))
        })?;

        let device = DeviceProfile::new(
            Some(identity),
            Some(device.name),
            device.app
        );

        #[derive(Deserialize, Debug)]
        #[allow(non_snake_case)]
        struct PeerInfo {
            peerId: Id,
            nodeId: Id,
            port: u16,
            apiUrl: Option<String>,
            #[serde(with = "crate::serde_bytes_base64")]
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
        self.repo.as_mut().map(|v| {
            v.put_message(message).map_err(|e| {
                error!("Save message failed, error: {e}");
            });
        });
    }
}

impl ContactListener for UserAgent {
    fn on_contacts_updating(&self,
        _version_id: &str,
        _contacts: Vec<Contact>
    ) {
        // TODO: implement this method
    }

    fn on_contacts_updated(&self,
        _base_version_id: &str,
        _new_version_id: &str,
        _contacts: Vec<Contact>
    ) {
        // TODO: implement this method
    }

    fn on_contacts_cleared(&self) {
        // TODO: implement this method
    }

    fn on_contact_profile(&self,
        _contact_id: &Id,
        _profile: &Contact
    ) {
        // TODO: implement this method
    }
}

impl ChannelListener for UserAgent {
    fn on_joined_channel(&self, _channel: &Channel) {
        println!("on_joined_channel called");
    }

    fn on_left_channel(&self, _channel: &Channel) {
        unimplemented!()
    }

    fn on_channel_deleted(&self, _channel: &Channel) {
        println!("on_channel_deleted called");
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

impl MessageListenerMut for UserAgent {
    fn on_message(&mut self, mut message: Message) {
        let conv_id = match !self.is_myself(message.to()) {
            true => message.to(),
            false => message.from(),
        }.clone();

        message.set_conversation_id(&conv_id);
        for cb in self.message_listeners.iter_mut() {
            cb.on_message(&message);
        }
        self.put_message(message);
        // TODO: self.get_or_create_conversation(conv_id).update(_message);
    }

    fn on_sending(&mut self, mut message: Message) {
        let conv_id = message.to().clone();
        message.set_conversation_id(&conv_id);
        for cb in self.message_listeners.iter_mut() {
            cb.on_sending(&message);
        };
        self.put_message(message);
        // TODO: self.get_or_create_conversation(conv_id).update(_message);
    }

    fn on_sent(&mut self, _message: Message) {
        unimplemented!()
    }

    fn on_broadcast(&mut self, _message: Message) {
        unimplemented!()
    }
}

impl ProfileListenerMut for UserAgent {
    fn on_user_profile_acquired(&mut self, profile: UserProfile) {
        if let Some(ref user) = self.user {
            if user.id() != profile.id() {
                warn!("User profile acquired with different id: {} != {}", user.id(), profile.id());
            }
        }

        self.user = Some(profile);
        self.update_userinfo_config();
        for cb in self.profile_listeners.iter_mut() {
            cb.on_user_profile_acquired(self.user.as_ref().unwrap());
        }
    }

    fn on_user_profile_changed(&mut self, name: &str, avatar: bool) {
        let Some(ref user) = self.user else {
            warn!("User profile is not set!");
            return;
        };

        self.user = Some(UserProfile::new(
            user.identity().clone(),
            name.to_string(),
            avatar
        ));

        self.update_userinfo_config();
        for cb in self.profile_listeners.iter_mut() {
            cb.on_user_profile_changed(name, avatar);
        }
    }
}

impl ConnectionListener for UserAgent {
    fn on_connecting(&self) {
        self.connection_listeners.iter().for_each(|l| {
            l.on_connecting();
        });
    }

    fn on_connected(&self) {
        self.connection_listeners.iter().for_each(|l| {
            l.on_connected();
        });
    }

    fn on_disconnected(&self) {
        self.connection_listeners.iter().for_each(|l| {
            l.on_disconnected();
        });
    }
}

impl MessageListener for UserAgent {
    fn on_message(&self, message: &Message) {
        for cb in self.message_listeners.iter() {
            cb.on_message(message);
        }
    }

    fn on_sending(&self, message: &Message) {
        for cb in self.message_listeners.iter() {
            cb.on_sending(message);
        };
    }

    fn on_sent(&self, message: &Message) {
        for cb in self.message_listeners.iter() {
            cb.on_sent(message);
        };
    }

    fn on_broadcast(&self, message: &Message) {
        for cb in self.message_listeners.iter() {
            cb.on_broadcast(message);
        };
    }
}

impl ProfileListener for UserAgent {
    fn on_user_profile_acquired(&self, profile: &UserProfile) {
        for cb in self.profile_listeners.iter() {
            cb.on_user_profile_acquired(profile);
        }
    }

    fn on_user_profile_changed(&self, name: &str, avatar: bool) {
        for cb in self.profile_listeners.iter() {
            cb.on_user_profile_changed(name, avatar);
        }
    }
}

unsafe impl Send for UserAgent {}
unsafe impl Sync for UserAgent {}

impl UserAgentCaps for UserAgent {
    fn user(&self) -> Option<&UserProfile> {
        self.user.as_ref()
    }

    fn device(&self) -> Option<&DeviceProfile> {
        self.device.as_ref()
    }

    fn peer(&self) -> &PeerInfo {
        assert!(self.peer.is_some(), "Peer info is not set!");
        self.peer.as_ref().unwrap()
    }

    fn is_configured(&self) -> bool {
        self.user.is_some() &&
            self.device.is_some() &&
            self.peer.is_some() &&
            //self.repository.is_some() &&
            self.peer().is_valid() &&
            self.peer().alternative_url().is_some()
    }

    fn harden(&mut self) {
        self.hardened = true;
    }

    fn add_connection_listener(&mut self, listener: Box<dyn ConnectionListener>) {
        self.connection_listeners.push(listener);
    }

    fn add_profile_listener(&mut self, listener: Box<dyn ProfileListener>) {
        self.profile_listeners.push(listener);
    }

    fn add_message_listener(&mut self, listener: Box<dyn MessageListener>) {
        self.message_listeners.push(listener);
    }

    fn add_channel_listener(&mut self, listener: Box<dyn ChannelListener>) {
        self.channel_listeners.push(listener);
    }

    fn add_contact_listener(&mut self, listener: Box<dyn ContactListener>) {
        self.contact_listeners.push(listener);
    }

    fn conversation(&self, conversation_id: &Id) -> Option<&Conversation> {
        self.conversations.get(conversation_id)
    }

    fn conversations(&self) -> Vec<&Conversation> {
        self.conversations.values().collect()
    }

    fn remove_conversation(&mut self, conversation_id: &Id) {
        self.conversations.remove(conversation_id);
        if let Some(repo) = self.repo.as_mut() {
            let _ = repo.remove_messages_by_conversation(conversation_id);
        }
    }

    fn remove_conversations(&mut self, conversation_ids: Vec<&Id>) {
        for id in conversation_ids {
            self.remove_conversation(id);
        }
    }

    fn messages(&self, converstation_id: &Id) -> Vec<Message> {
        self.repo.as_ref().map(|v| {
            v.messages_since(converstation_id, 0, 100, 0)
        })
        .unwrap_or_else(|| Ok(vec![]))
        .unwrap()
    }

    fn messages_between(&self, converstation_id: &Id, from: u64, end: u64) -> Vec<Message> {
        self.repo.as_ref().map(|v| {
            v.messages_between(converstation_id, from, end)
        })
        .unwrap_or_else(|| Ok(vec![]))
        .unwrap()
    }

    fn messages_since(&self, converstation_id: &Id, since: u64, limit: usize, offset: usize) -> Vec<Message> {
        self.repo.as_ref().map(|v| {
            v.messages_since(converstation_id, since, limit, offset)
        })
        .unwrap_or_else(|| Ok(vec![]))
        .unwrap()
    }

    fn remove_message(&mut self, messsage_id: u32) {
        self.repo.as_mut().map(|v| {
            _ = v.remove_amessage(messsage_id);
        });
    }

    fn remove_messages(&mut self, message_ids: &[u32]) {
        self.repo.as_mut().map(|v| {
            _ = v.remove_messages(message_ids);
        });
    }

    fn remove_messages_by_conversation(&mut self, converstation_id: &Id) {
        self.repo.as_mut().map(|v| {
            _ = v.remove_messages_by_conversation(converstation_id);
        });
    }

    fn channels(&self) -> Result<Vec<&Channel>> {
        unimplemented!()
    }

    fn channel(&self, _channel_id: &Id) -> Result<Option<Channel>> {
        // TODO: implement channel retrieval logic.
        let channel_data = ChannelData::new(
            Id::random(),
            channel::Permission::OwnerInvite,
            Some("test channel".into())
        );

        let channel = GenericContact::new(
            Id::random(),
            Id::random(),
            channel_data
        );
        Ok(Some(channel))
    }

    fn contacts_version(&self) -> Result<String> {
        // TODO: Implement contact version retrieval logic.
        Ok("v1.0.0".into())
    }

    fn put_contacts_update(&mut self, _version_id: &str, _contacts: &[Contact]) -> Result<()> {
        //unimplemented!()
        Ok(())
    }

    fn contact(&self, _id: &Id) -> Result<Option<Contact>> {
        Ok(None)
    }

    fn contacts(&self) -> Result<Vec<Contact>> {
        Ok(vec![])
    }

    fn remove_contact(&mut self, _id: &Id) -> Result<()> {
        Ok(())
    }
    fn remove_contacts(&mut self, _ids: Vec<&Id>) -> Result<()> {
        Ok(())
    }
}
