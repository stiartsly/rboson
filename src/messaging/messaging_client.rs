
use crate::{
    Id,
    error::Result,
};

use crate::messaging::{
    user_agent::UserAgent,
    channel::{Role, Permission, Channel},
    client_device::ClientDevice,
    invite_ticket::InviteTicket,
    contact::Contact,
};

#[allow(unused)]
pub(crate) trait MessagingClient {
    fn userid(&self) -> &Id;
    fn user_agent(&self) -> &Box<dyn UserAgent>;

    async fn close(&mut self) -> Result<()>;
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;

    async fn update_profile(&mut self, name: &str, avatar: bool) -> Result<()>;
    async fn upload_avatar(&mut self, content_type: &str, avatar: &[u8]) -> Result<String>;
    async fn upload_avatar_with_filename(&mut self, content_type: &str, file_name: &str) -> Result<String>;

    async fn devices(&self) -> Result<Vec<ClientDevice>>;
    async fn revoke_device(&mut self, device_id: &Id) -> Result<()>;

    async fn create_channel(&mut self, name: &str, notice: Option<&str>) -> Result<Channel>;
    async fn create_channel_with_permission(&mut self, permission: &Permission, name: &str, notice: Option<&str>) -> Result<Channel>;
    async fn remove_channel(&mut self, channel_id: &Id) -> Result<()>;
    async fn join_channel(&mut self, ticket: &InviteTicket) -> Result<()>;
    async fn leave_channel(&mut self, channel_id: &Id) -> Result<()>;

    async fn create_invite_ticket(&mut self, channel_id: &Id) -> Result<InviteTicket>;
    async fn create_invite_ticket_with_invitee(&mut self, channel_id: &Id, invitee: &Id) -> Result<InviteTicket>;

    async fn set_channel_owner(&mut self, channel_id: &Id, new_owner: &Id) -> Result<()>;
    async fn set_channel_permission(&mut self, channel_id: &Id, permission: &Permission) -> Result<()>;
    async fn set_channel_name(&mut self, channel_id: &Id, name: &str) -> Result<()>;
    async fn set_channel_notice(&mut self, channel_id: &Id, notice: &str) -> Result<()>;

    async fn set_channel_member_role(&mut self, channel_id: &Id, members: Vec<&Id>, role: &Role) -> Result<()>;
    async fn ban_channel_members(&mut self, channel_id: &Id, members: Vec<&Id>) -> Result<()>;
    async fn unban_channel_members(&mut self, channel_id: &Id, members: Vec<&Id>) -> Result<()>;
    async fn remove_channel_members(&mut self, channel_id: &Id, members: Vec<&Id>) -> Result<()>;


    async fn channel(&self, id: &Id) -> Result<&Channel>;

    async fn contact(&self, id: &Id) -> Result<&Contact>;
    async fn contacts(&self) -> Result<Vec<&Contact>>;
    async fn add_contact(&mut self, id: &Id, home_peer_id: Option<&Id>, session_key: &[u8], remark: Option<&str>) -> Result<()>;
    async fn update_contact(&mut self, contact: Contact) -> Result<()>;
    async fn remove_contact(&mut self, id: &Id) -> Result<()>;
    async fn remove_contacts(&mut self, ids: Vec<&Id>) -> Result<()>;
}
