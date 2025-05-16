use super::message::Message;

pub trait MessageListener {
    fn on_message(&self, message: &Message);
    fn on_sending(&self, message: &Message);
    fn on_sent(&self, message: &Message);
    fn on_broadcast(&self, message: &Message);
}
