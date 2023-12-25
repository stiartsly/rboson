use crate::node_info::NodeInfo;

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

    pub(crate) fn push(&mut self, node: &NodeInfo) {
        self.nodes.push(node.clone());
        self.updated = true;
    }

    pub(crate) fn push_nodes(&mut self, nodes: &[NodeInfo]) {
        nodes.iter().for_each(|item| {
            self.nodes.push(item.clone());
        });
        self.updated = true;
    }

    pub(crate) fn pop_all<F>(&mut self, f: F)
    where F: Fn(NodeInfo) {
        if !self.updated || self.nodes.is_empty() {
            return;
        }

        while let Some(item) = self.nodes.pop() {
            f(item);
        }
        self.updated = false;
    }
}
