use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeStatus {
    Stopped,
    Initializing,
    Running,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            NodeStatus::Stopped => "Stopped",
            NodeStatus::Initializing => "Initializing",
            NodeStatus::Running => "Running"
        };
        write!(f, "{}", str)
    }
}
