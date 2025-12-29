use crate::messaging::{
    channel::{Channel, Member, Role},
};

#[allow(dead_code)]
pub(crate) trait ChannelListenerMut {
    fn on_joined_channel(&mut self, channel: &Channel);
    fn on_left_channel(&mut self, channel: &Channel);
    fn on_channel_deleted(&mut self, channel: &Channel);
    fn on_channel_updated(&mut self, channel: &Channel);

    fn on_channel_members(&mut self,
        channel: &Channel,
        members: &[Member]
    );

    fn on_channel_member_joined(&mut self,
        channel: &Channel,
        member: &Member
    );

    fn on_channel_member_left(&mut self,
        channel: &Channel,
        member: &Member
    );

    fn on_channel_members_removed(&mut self,
        channel: &Channel,
        members: &[Member]
    );

    fn on_channel_members_banned(&mut self,
        channel: &Channel,
        banned: &[Member]
    );

    fn on_channel_members_unbanned(&mut self,
        channel: &Channel,
        unbanned: &[Member]
    );

    fn on_channel_members_role_changed(
        &mut self,
        channel: &Channel,
        changed: &[Member],
        role: Role,
    );
}

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
