use super::{
    user_profile::UserProfile,
};

#[allow(unused)]
pub trait ProfileListener {
    fn on_user_profile_acquired(&self, _: &UserProfile);
    fn on_user_profile_changed(&self, _: bool);
}
