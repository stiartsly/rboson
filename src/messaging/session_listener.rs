use crate::messaging::session_info::SessionInfo;

/// Receives events about device session lifecycle.
pub trait SessionListener: Send + Sync {
    /// Called when a new device session is established.
    fn on_new_session(&self, session_info: &SessionInfo);
}
