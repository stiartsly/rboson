use crate::messaging::{
    channel::{Channel, Member, Role},
};

pub trait ChannelListener {
    fn on_joined_channel(&self, channel: &Channel);
    fn on_left_channel(&self, channel: &Channel);
    fn on_channel_deleted(&self, channel: &Channel);
    fn on_channel_updated(&self, channel: &Channel);

    fn on_channel_members(&self,
        channel: &Channel,
        members: &[Member]
    );

    fn on_channel_member_joined(&self,
        channel: &Channel,
        member: &Member
    );

    fn on_channel_member_left(&self,
        channel: &Channel,
        member: &Member
    );

    fn on_channel_members_removed(&self,
        channel: &Channel,
        members: &[Member]
    );

    fn on_channel_members_banned(&self,
        channel: &Channel,
        banned: &[Member]
    );

    fn on_channel_members_unbanned(&self,
        channel: &Channel,
        unbanned: &[Member]
    );

    fn on_channel_members_role_changed(
        &self,
        channel: &Channel,
        changed: &[Member],
        role: Role,
    );
}
