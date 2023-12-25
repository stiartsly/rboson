use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LookupOption {
    Local,
    Arbitrary,
    Optimistic,
    Conservative,
}

impl fmt::Display for LookupOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            LookupOption::Local => "Local",
            LookupOption::Arbitrary => "Arbitrary",
            LookupOption::Optimistic => "Optimistic",
            LookupOption::Conservative => "Conservative",
        };
        write!(f, "{}", str)
    }
}
