use crate::{
    Id,
    Identity,
};

pub trait GroupIdentity: Identity {
    fn member_publickey(&self) -> &Id;
}
