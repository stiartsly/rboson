use std::fmt;
use crate::msg::msg;

pub(crate) struct Stats {
    // TODO;
}

impl Stats {
    pub(crate) fn new() -> Self {
        Stats {}
    }

    pub(crate) fn received_bytes(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn sent_bytes(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn received_bytes_in_sec(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn sent_bytes_in_sec(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn received_msgs(&self, _: msg::Method, _: msg::Kind) -> usize {
        unimplemented!()
    }

    pub(crate) fn total_received_msgs(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn sent_msgs(&self, _: msg::Method, _: msg::Kind) -> usize {
        unimplemented!()
    }

    pub(crate) fn total_sent_msgs(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn timeout_msgs(&self, _: msg::Method) -> usize {
        unimplemented!()
    }

    pub(crate) fn total_timeout_msgs(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn dropped_packets(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn dropped_bytes(&self) -> usize {
        unimplemented!()
    }

    pub(crate) fn on_received_bytes(&mut self, _: usize) {
        unimplemented!()
    }

    pub(crate) fn on_sent_bytes(&mut self, _: usize) {
        unimplemented!()
    }

    pub(crate) fn on_received_msg(&mut self, _: &Box<dyn msg::Msg>) {
        unimplemented!()
    }

    pub(crate) fn on_sent_msg(&mut self, _: &Box<dyn msg::Msg>) {
        unimplemented!()
    }

    pub(crate) fn on_timeout_msg(&mut self, _: &Box<dyn msg::Msg>) {
        unimplemented!()
    }

    pub(crate) fn on_dropped_packet(&mut self, _: usize) {
        unimplemented!()
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
