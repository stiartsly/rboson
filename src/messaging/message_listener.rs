use super::message::Message;

pub trait MessageListener {
    fn on_message<T, R>(&self, msg: &Message<T,R>);
    fn on_sent<T, R>(&self, msg: &Message<T,R>);
    fn on_broadcast<T, R>(&self, msg: &Message<T,R>);
}
