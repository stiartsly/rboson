use super::message::Message;

#[allow(unused)]
pub(crate) trait MessageListenerMut {
    fn on_message(&mut self, message: Message);
    fn on_sending(&mut self, message: Message);
    fn on_sent(&mut self, message: Message);
    fn on_broadcast(&mut self, message: Message);
}

pub trait MessageListener {
    fn on_message(&self, message: &Message);
    fn on_sending(&self, message: &Message);
    fn on_sent(&self, message: &Message);
    fn on_broadcast(&self, message: &Message);
}
