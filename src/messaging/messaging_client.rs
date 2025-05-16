
use crate::{
    Id,
    error::Result,
};

#[allow(unused)]
pub(crate) trait MessagingClient {
    fn user_id(&self) -> &Id;
    fn device_id(&self) -> &Id;

    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;

    async fn close(&mut self) -> Result<()>;
    async fn revoke_device(&mut self, device_id: &Id) -> Result<()>;
}
