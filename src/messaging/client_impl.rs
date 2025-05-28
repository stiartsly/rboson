use unicode_normalization::UnicodeNormalization;
use url::Url;

use crate::{
    Id,
    error::Result,
    Identity,
    messaging::ServiceIds,
};

use super::{
    messaging_client::{Builder, MessagingClient},
    api_client::{self, APIClient},

    user_agent::UserAgent,
    client_device::ClientDevice,
    channel::{Role, Permission, Channel},
    invite_ticket::InviteTicket,
    contact::Contact,
};

#[allow(dead_code)]
pub struct Client {
    userid:   Id,
    dev_id: Id,

    api_client: APIClient,
}

#[allow(dead_code)]
impl Client {
    pub(crate) fn new(b: &Builder) -> Result<Self> {
        Ok(Self {
            userid: b.user().unwrap().id().clone(),
            dev_id: b.device().as_ref().unwrap().id().clone(),

            api_client: api_client::Builder::new()
                .with_base_url(b.api_url().as_ref().unwrap().as_str())
                .with_home_peerid(b.peerid().as_ref().unwrap())
                .with_user_identity(b.user().as_ref().unwrap())
                .with_device_identity(b.device().as_ref().unwrap())
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
