use crate::NodeInfo;

pub(crate) struct BootstrapChannel {
    nodes: Vec<NodeInfo>,
    updated: bool,
}

impl BootstrapChannel {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            updated: false,
        }
    }

    pub(crate) fn push(&mut self, ni: NodeInfo) {
        self.nodes.push(ni);
        self.updated = true;
    }

    pub(crate) fn push_nodes(&mut self, nis: Vec<NodeInfo>) {
        self.nodes.extend(nis);
        self.updated = true;
    }

    pub(crate) fn pop_all<F>(&mut self, cb: F) where F: Fn(NodeInfo) {
        if !self.updated {
            return;
        }
        if self.nodes.is_empty() {
            return;
        }

        while let Some(item) = self.nodes.pop() {
            cb(item);
        }
        self.updated = false;
    }
}
