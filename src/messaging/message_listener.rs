use crate::messaging::message::Message;

/// Receives message delivery events.
pub trait MessageListener: Send + Sync {
    /// Called when a new inbound message arrives.
    fn on_message(&self, message: &dyn Message);

    /// Called when an outbound message was successfully delivered.
    fn on_sent(&self, _message: &dyn Message) {}
}
