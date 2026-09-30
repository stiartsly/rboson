use crate::messaging::SessionInfo;

/// A listener interface for monitoring the creation of new sessions.
/// Classes that are interested in being notified about new sessions
/// should implement this interface and register with a session management
/// mechanism.
pub trait SessionListener: Send + Sync {

    /// Called when a new session is created.
    /// This method notifies the implementing class of newly established sessions,
    /// providing session details for further processing or tracking.
    ///
    /// # Arguments
    ///
    /// * `session_info` - An object representing the information related to the new session,
    ///                    including its unique identifier, online status, and timestamp of the last activity.
    fn on_new_session(&self, session_info: &SessionInfo);
}
