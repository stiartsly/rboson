use super::message::Message;

#[allow(dead_code)]
pub(crate) trait MessageListener {
    fn on_message(&self, msg: &Message);
    fn on_sent(&self, msg: &Message);
    fn on_broadcast(&self, msg: &Message);
}
