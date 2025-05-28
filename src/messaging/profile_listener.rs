use super::{
    user_profile::UserProfile,
};

pub trait ProfileListener {
    fn on_user_profile_acquired(&self, profile: &UserProfile);
    fn on_user_profile_changed(&self, avatar: bool);
}
