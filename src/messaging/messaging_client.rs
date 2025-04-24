
use crate::{
    Id,
    error::Result,
};

use super::{
    group::Group,
    // group_permission::GroupPermission,
    // client_device::ClientDevice,
    // invite_ticket::InviteTicket,
};

#[allow(unused)]
pub(crate) trait MessagingClient {
    fn user_id(&self) -> &Id;
    fn device_id(&self) -> &Id;

    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;

    async fn close(&mut self) -> Result<()>;

    // async fn list_devices(&self) -> Result<Vec<ClientDevice>>;
    async fn revoke_device(&mut self, device_id: &Id) -> Result<()>;

    //async fn create_group(&mut self, _name: &str, _notice: &str, _permission: Option<GroupPermission>) -> Result<Group>;
    async fn create_group_simple(&mut self, _name: &str, _notice: &str) -> Result<Group> {
        unimplemented!()
    }

    //async fn join_group(&mut self, ticket: &InviteTicket, private_key: Vec<u8>) -> Result<Group>;
    //async fn remove_group(&mut self, group_id: &Id) -> Result<()>;
}
