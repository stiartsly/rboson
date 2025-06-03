use unicode_normalization::UnicodeNormalization;
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
};

use super::{
    messaging_client::{Builder, MessagingClient},
    api_client::{APIClient, Builder as APIClientBuilder},

    client_device::ClientDevice,
    channel::{Role, Permission, Channel},
    invite_ticket::InviteTicket,
    contact::Contact,
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
