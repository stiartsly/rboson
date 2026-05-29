/// Receives connection lifecycle events from the messaging client.
pub trait ConnectionListener: Send + Sync {
    /// Called when the client has started the connection attempt.
    fn on_connecting(&self) {}

    /// Called when the low-level connection is established.
    fn on_connected(&self) {}

    /// Called when the connection is fully initialised and ready for use.
    fn on_ready(&self);

    /// Called when the connection drops or is closed.
    fn on_disconnected(&self) {}
}
