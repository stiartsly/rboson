use std::sync::{Arc, Mutex};
use std::future::Future;
use crate::{
    Id,
    core::Result,
};

use crate::messaging::{
    UserAgent,
    Role,
    Permission,
    Channel,
    InviteTicket,
    Contact,
    client_device::ClientDevice
};

// messaging capabilities trait
pub trait MessagingAgent{
    fn userid(&self) -> &Id;
    fn user_agent(&self) -> Arc<Mutex<UserAgent>> ;

    fn close(&mut self) -> impl Future<Output = Result<()>>;
    fn connect(&mut self) -> impl Future<Output = Result<()>>;
    fn disconnect(&mut self) -> impl Future<Output = Result<()>>;
    fn is_connected(&self) -> bool;

    //fn message(&mut self) -> MessageBuilder;

    fn update_profile(&mut self,
        name: Option<&str>,
        avatar: bool
    ) -> impl Future<Output = Result<()>>;

    fn upload_avatar(&mut self,
        content_type: &str,
        avatar: &[u8]
    ) -> impl Future<Output = Result<String>>;

    fn upload_avatar_from_file(&mut self,
        content_type: &str,
        file_name: &str
    ) -> impl Future<Output = Result<String>>;

    fn devices(&mut self) -> impl Future<Output = Result<Vec<ClientDevice>>>;

    fn revoke_device(&mut self,
        device_id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn create_channel(&self,
        permission: Option<Permission>,
        name: &str,
        notice: Option<&str>
    ) -> impl Future<Output = Result<Channel>>;

    fn remove_channel(&self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn join_channel(&mut self,
        ticket: &InviteTicket
    ) -> impl Future<Output = Result<()>>;

    fn leave_channel(&mut self,
        channel_id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn create_invite_ticket(&mut self,
        channel_id: &Id,
        invitee: Option<&Id>
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
        members: Vec<&Id>
    ) -> impl Future<Output = Result<()>>;

    fn unban_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<&Id>
    ) -> impl Future<Output = Result<()>>;

    fn remove_channel_members(&mut self,
        channel_id: &Id,
        members: Vec<&Id>
    ) -> impl Future<Output = Result<()>>;

    fn contact(&self, id: &Id) -> impl Future<Output = Result<Option<Contact>>>;

    fn channel(&self, id: &Id) -> impl Future<Output = Result<Option<Channel>>>;

    fn contacts(&self) ->impl Future<Output = Result<Vec<Contact>>>;

    fn add_contact(&mut self,
        id: &Id,
        home_peer_id: Option<&Id>,
        session_key: &[u8],
        remark: Option<&str>
    ) -> impl Future<Output = Result<Contact>>;

    fn update_contact(&mut self,
        contact: Contact
    ) -> impl Future<Output = Result<Contact>>;

    fn remove_contact(&mut self,
        id: &Id
    ) -> impl Future<Output = Result<()>>;

    fn remove_contacts(&mut self,
        ids: Vec<&Id>
    ) -> impl Future<Output = Result<()>>;
}
