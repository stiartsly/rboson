pub trait ConnectionListener {
    fn on_connecting(&self) {}
    fn on_connected(&self) {}
    fn on_disconnected(&self) {}
}
