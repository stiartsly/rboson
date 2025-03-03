use crate::{
    Id
};

use super::{
    group::Group,
    group_member::GroupMember,
};

pub trait GroupAdapter {
    fn group(&self, group_id: &Id) -> Option<&Group>;
    fn all_groups(&self) -> Vec<&Group>;
    fn exists(&self, group_id: &Id) -> bool {
        self.group(group_id).is_some()
    }

    // Myself joined a new group
	// The group object already include the member private key
    fn on_joined_group(&self, group: &Group);
    fn on_leave_group(&self, group_id: &Id);

	// THe group has a new member joined.
    fn on_group_member_joined(&self, group_id: &Id, member: &GroupMember);
    fn on_group_member_left(&self, group_id: &Id, member_id: &Id);
    fn on_group_member_removed(&self, group_id: &Id, removed: &[&Id]);
    fn on_group_member_banned(&self, group_id: &Id, banned: &[u8]);
    fn on_group_member_unbanned(&self, group_id: &Id, unbanned: &[u8]);

    fn on_group_updated(&self, group_id: &Id, name: &str, notice: &str);
    fn on_group_deleted(&self, group_id: &Id);
}
