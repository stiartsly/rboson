use std::collections::HashMap;

use crate::{
    Id,
    signature,
    cryptobox,
    error::Result,
};

use crate::core::{
    identity::Identity
};

use super::{
    group_permission::GroupPermission,
    group_member::GroupMember,
    group_role::GroupRole,
};

pub trait GroupIdentity: Identity {
    fn member_publickey(&self) -> &Id;
}

#[allow(dead_code)]
pub struct Group {
    keypair: signature::KeyPair,
    member_publickey: Id,
    encryption_keypair: cryptobox::KeyPair,

    owner: Id,
    permission: GroupPermission,
    notice: Option<String>,

    members: HashMap<Id, GroupMember>,
}

#[allow(dead_code)]
impl Group {
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|v|v.as_str())
    }

    pub fn set_notice(&mut self, notice: String) {
        self.notice = Some(notice);
        self.touch();
    }

    pub fn owner(&self) -> &Id {
        &self.owner
    }

    pub fn set_owner(&mut self, owner: Id) {
        self.owner = owner;
        self.touch();
    }

    pub fn permission(&self) -> &GroupPermission {
        &self.permission
    }

    pub fn set_permission(&mut self, permission: GroupPermission) {
        self.permission = permission;
        self.touch();
    }

    pub fn set_blocked(&mut self, _blocked: bool) {
        // Do nothing on group contact.
    }

    pub fn member_publickey(&self) -> &Id {
        &self.member_publickey
    }

    pub fn touch(&mut self) {
        unimplemented!()
    }

    pub fn sign(&self, _data: Vec<u8>) -> Result<Vec<u8>> {
        unimplemented!()
    }

    pub fn verify(&self, _data: &[u8], _signature: &[u8]) -> bool {
        unimplemented!()
    }



    pub fn size(&self) -> usize {
        self.members.len()
    }

    pub fn members(&self) -> Vec<&GroupMember> {
        self.members.values().collect()
    }

    pub fn is_owner(&self, id: &Id) -> bool {
        id == &self.owner
    }

    pub fn is_moderator(&self, id: &Id) -> bool {
        self.members.get(id).map_or(false, |v| {
            v.role() == &GroupRole::Moderator
        })
    }

    pub fn is_banned(&self, id: &Id) -> bool {
        self.members.get(id).map_or(false, |v| {
            v.role() == &GroupRole::Banned
        })
    }

    pub fn is_member(&self, id: &Id) -> bool {
        self.members.get(id).map_or(false, |v| {
            v.role() != &GroupRole::Banned
        })
    }

    pub fn is_qualified_inviter(&self, inviter: &Id) -> bool {
        let Some(member) = self.members.get(inviter) else {
            return false;
        };

        let role = member.role();
        match self.permission {
            GroupPermission::Public | GroupPermission::MemberInvite => {
                role <= &GroupRole::Member
            },
            GroupPermission::ModeratorInvite => {
                role <= &GroupRole::Moderator
            },
            GroupPermission::OwnerInvite => {
                inviter == &self.owner
            }
        }
    }
}

impl Identity for Group {
    fn id(&self) -> &Id {
        unimplemented!()
    }

    fn sign_into(&self, _data: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn verify(&self, _data: &[u8], _signature: &[u8]) -> Result<()> {
        unimplemented!()
    }

    fn encrypt_into(&self, _recipient: &Id, _data: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn decrypt_into(&self, _sender: &Id, _data: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }
}

impl GroupIdentity for Group {
    fn member_publickey(&self) -> &Id {
        &self.member_publickey
    }
}
