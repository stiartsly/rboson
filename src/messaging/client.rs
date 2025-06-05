use std::future::Future;
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

pub trait MessagingClient {
    fn userid(&self) -> &Id;
    fn user_agent(&self) -> &Box<dyn UserAgent>;

    fn close(&mut self) -> impl Future<Output = Result<()>>;
    fn connect(&mut self) -> impl Future<Output = Result<()>>;

    fn disconnect(&mut self) -> impl Future<Output = Result<()>>;
    fn is_connected(&self) -> bool;

    fn update_profile(&mut self,
        name: &str,
        avatar: bool
    ) -> impl Future<Output = Result<()>>;

    fn upload_avatar(&mut self,
        content_type: &str,
        avatar: &[u8]
    ) -> impl Future<Output = Result<String>>;

    fn upload_avatar_with_filename(&mut self,
        content_type: &str,
        file_name: &str
    ) -> impl Future<Output = Result<String>>;

    fn devices(&self) -> impl Future<Output = Result<Vec<ClientDevice>>>;

    fn revoke_device(&mut self,
        device_id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn create_channel(&mut self,
        name: &str,
        notice: Option<&str>
    ) -> impl Future<Output = Result<Channel>>;

    fn create_channel_with_permission(&mut self,
        permission: &Permission,
        name: &str,
        notice: Option<&str>
    ) -> impl Future<Output = Result<Channel>>;

    fn remove_channel(&mut self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn join_channel(&mut self,
        ticket: &InviteTicket
    ) -> impl Future<Output = Result<()>>;

    fn leave_channel(&mut self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn create_invite_ticket(&mut self,
        channel_id: &Id
    ) -> impl Future<Output = Result<InviteTicket>>;

    fn create_invite_ticket_with_invitee(&mut self,
        channel_id: &Id,
        invitee: &Id
    ) -> impl Future<Output = Result<InviteTicket>>;

    fn set_channel_owner(&mut self,
        channel_id: &Id,
        new_owner: &Id
    ) -> impl Future<Output = Result<()>>;

    fn set_channel_permission(&mut self,
        channel_id: &Id,
        permission: Permission
    ) -> impl Future<Output = Result<()>>;

    fn set_channel_name(&mut self,
        channel_id: &Id,
        name: Option<&str>
    ) -> impl Future<Output = Result<()>>;

    fn set_channel_notice(&mut self,
        channel_id: &Id,
        notice: Option<&str>
    ) -> impl Future<Output = Result<()>>;

    fn set_channel_member_role(&mut self,
        channel_id: &Id,
        members: Vec<&Id>,
        role: Role
    ) -> impl Future<Output = Result<()>>;

    fn ban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<Id>
    ) -> impl Future<Output = Result<()>>;

    fn unban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<Id>
    ) -> impl Future<Output = Result<()>>;

    fn remove_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<Id>
    ) -> impl Future<Output = Result<()>>;

    fn channel(&self, id: &Id) -> impl Future<Output = Result<&Channel>>;

    fn contact(&self, id: &Id) -> impl Future<Output = Result<&Contact>>;

    fn contacts(&self) ->impl Future<Output = Result<Vec<&Contact>>>;

    fn add_contact(&mut self,
        id: &Id,
        home_peer_id: Option<&Id>,
        session_key: &[u8],
        remark: Option<&str>
    ) -> impl Future<Output = Result<()>>;

    fn update_contact(&mut self,
        contact: Contact
    ) -> impl Future<Output = Result<()>>;

    fn remove_contact(&mut self,
        id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn remove_contacts(&mut self,
        ids: Vec<&Id>
    ) -> impl Future<Output = Result<()>>;
}
