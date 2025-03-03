
use std::time::SystemTime;
use std::fmt;

use crate::Id;

#[allow(dead_code)]
pub struct InviteTicket {
    group_id:   Id,
    inviter:    Id,

    open_to_anyone: bool,
    expire:     SystemTime,
    signature:  Vec<u8>
}

impl InviteTicket {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub fn group_id(&self) -> &Id {
        &self.group_id
    }

    pub fn inviter(&self) -> &Id {
        &self.inviter
    }

    pub fn is_expired(&self) -> bool {
        self.expire < SystemTime::now()
    }

    pub fn is_valid(&self, _invitee: &Id) -> bool {

        unimplemented!()
    }
}

impl fmt::Display for InviteTicket {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}
