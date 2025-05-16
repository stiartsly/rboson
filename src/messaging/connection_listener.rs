
pub trait ConnectionListener {
    fn on_connection(&self) {}
    fn on_connected(&self) {}
    fn on_disconnected(&self) {}
}