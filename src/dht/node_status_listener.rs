use crate::dht::{
    node_status::NodeStatus,
};

pub trait NodeStatusListener {
    fn status_changed(&self,
        _new_status: NodeStatus,
        _old_status: NodeStatus,
    ) {}

    fn starting(&self) {}
    fn started(&self) {}
    fn stopping(&self) {}
    fn stopped(&self) {}
}
