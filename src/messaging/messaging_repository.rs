
use crate::{
    error::Result,
};

use super::{
    message::Message,
};

#[allow(unused)]
pub(crate) trait MessagingRepository {
    fn put_config(&self, _: &str, _: &[u8]) -> Result<()>;
    fn get_config(&self, _: &str) -> Result<Vec<u8>>;

    fn put_msg(&self, _:&Message) -> Result<()>;
    fn put_messages(&self, _: &[Message]) -> Result<()>;

    fn remove_msg(&self, _: u32);
}
