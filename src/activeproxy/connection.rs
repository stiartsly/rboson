
pub(crate) enum State {
    Initializing = 0,
    Authenticating,
    Attaching,
    Idling,
    Relaying,
    Disconnecting,
    Closed
}
