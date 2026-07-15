use crate::messaging::UserProfile;

#[allow(dead_code)]
pub(crate) trait ProfileListenerMut {
    fn on_user_profile_acquired(&mut self, profile: UserProfile);
    fn on_user_profile_changed(&mut self, name: &str, avatar: bool);
}

pub trait ProfileListener {
    fn on_user_profile_acquired(&self, profile: &UserProfile);
    fn on_user_profile_changed(&self, name: &str, avatar: bool);
}
