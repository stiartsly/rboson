use crate::{
    Id,
};

use super::{
    crypto_identity::Identity,
};

pub trait GroupIdentity: Identity {
    fn member_publickey(&self) -> &Id;
}
