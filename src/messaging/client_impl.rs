use unicode_normalization::UnicodeNormalization;
use serde::Serialize;
use sha2::{Digest, Sha256};
use url::Url;
use log::warn;

use crate::{
    Id,
    error::Result,
    Error,
    Identity,
    messaging::ServiceIds,
    core::crypto_identity::CryptoIdentity,
    core::crypto_context::CryptoContext,
    PeerInfo,
};

use crate::messaging::{
    DefaultUserAgent,
    UserAgent,
    InviteTicket,
    Contact,
    ClientBuilder as Builder,
    MessagingClient,
};

use super::{
    api_client::{APIClient, Builder as APIClientBuilder},

    client_device::ClientDevice,
    channel::{Role, Permission, Channel},

    rpc::request::RPCRequest,
    rpc::method::RPCMethod,
};

#[allow(dead_code)]
pub struct Client {

    peer            : PeerInfo,
    user            : CryptoIdentity,
    device          : CryptoIdentity,
    client_id       : String,

    inbox           : String,
    outbox          : String,

    self_context    : CryptoContext,
    server_context  : CryptoContext,

    api_client      : APIClient,

    user_agent      : Box<DefaultUserAgent>

}

#[allow(dead_code)]
impl Client {
    pub(crate) fn new(b: &mut Builder) -> Result<Self> {
        let mut agent = b.user_agent_take();
        if !agent.is_configured() {
            return Err(Error::State("User agent is not configured".into()));
        }

        agent.harden();

        let client_id: String = {
            // unimplemented!();
            "TODO".into()
        };

        let peer    = agent.peer_info().unwrap().clone();
        let user    = agent.user().unwrap().identity().clone();
        let device  = agent.device().unwrap().identity().unwrap().clone();

        let api_client = APIClientBuilder::new()
            .with_base_url(peer.alternative_url().unwrap())
            .with_home_peerid(peer.id())
            .with_user_identity(&user)
            .with_device_identity(&device)
            .build()?;

        Ok(Self {
            peer        : peer.clone(),
            user        : user.clone(),
            device      : device.clone(),

            client_id,

            inbox       : format!("inbox/{}", user.id().to_base58()),
            outbox      : format!("outbox/{}", user.id().to_base58()),

            user_agent  : agent,
            api_client,

            self_context    : user.create_crypto_context(user.id())?,
            server_context  : device.create_crypto_context(device.id())?
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        println!("Messaging client Started!");

        let version_id = self.user_agent.contact_version().unwrap_or_else(|_| {
            warn!("Fectching all contacts due to failed to get contacts version.");
            None
        });

        if version_id.is_none() {
            let update = self.api_client.fetch_contacts_update(
                version_id.as_ref().map(|v| v.as_str())
            ).await?;

            let Some(version_id) = update.version_id() else {
                return Err(Error::State("Contacts update does not contain version id".into()));
            };

            self.user_agent.put_contacts_update(version_id, update.contacts())
                .map_err(|e|
                    Error::State(format!("Failed to put contacts update: {}", e))
            )?;
        }

        //let


        Ok(())
    }

    pub fn stop(&self) {
        println!("Messaging client stopped");
    }

    pub async fn service_ids(url: &Url) -> Result<ServiceIds> {
        APIClient::service_ids(url).await
    }

    async fn sign_into_invite_ticket(&self, channel_id: &Id, invitee: Option<&Id>) -> Result<InviteTicket> {
        let Some(channel) = self.user_agent.channel(channel_id)? else {
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

    fn next_index(&mut self) -> u32 { 0 }
}

#[allow(dead_code)]
impl MessagingClient for Client {
    fn userid(&self) -> &Id {
        self.user.id()
    }

    fn user_agent(&self) -> &Box<dyn UserAgent> {
    //    &*self.user_agent
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
        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) {
            return Err(Error::State("Not channel owner".into()));
        }

        if channel.is_member(new_owner) {
            return Err(Error::State("New owner is not in the channel".into()));
        }

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
        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) {
            return Err(Error::State("Not channel owner".into()));
        }

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
        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }

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
        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }

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

        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }

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

        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }

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

        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }

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

        let Some(channel) = self.user_agent.channel(channel_id)? else {
            return Err(Error::State("Channel does not exist".into()));
        };

        if !channel.is_owner(channel_id) && !channel.is_moderator(channel_id) {
            return Err(Error::State("Not channel owner or moderator".into()));
        }

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
