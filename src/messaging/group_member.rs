
use std::time::SystemTime;

use crate::Id;
use super::{
    group_role::GroupRole,
};

#[allow(dead_code)]
pub struct GroupMember {
    id: Id,
    role: GroupRole,
    joined: SystemTime,
    last_modified: SystemTime
}

impl GroupMember {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn role(&self) -> &GroupRole {
        unimplemented!()
    }

    pub fn joined(&self) -> &SystemTime {
        &self.joined
    }

    pub fn last_modified(&self) -> &SystemTime {
        &self.last_modified
    }
}
