use std::time::SystemTime;
use crate::{
    Id,
};

#[allow(dead_code)]
pub(crate) struct Message {
    version:    i32,
    from:       Id,
    to:         Id,
    id:         i32,
    created:    SystemTime,
    msg_type:   i32,

    conversation_id:    Id,
}

#[allow(dead_code)]
impl Message {
    pub(crate) fn version(&self) -> i32 {
        self.version
    }

    pub(crate) fn conversation_id(&self) -> &Id {
        &self.conversation_id
    }

    pub(crate) fn from(&self) -> &Id {
        &self.from
    }

    pub(crate) fn to(&self) -> &Id {
        &self.to
    }

    pub(crate) fn id(&self) -> i32 {
        self.id
    }

    pub(crate) fn message_type(&self) -> i32 {
        self.msg_type
    }
}
