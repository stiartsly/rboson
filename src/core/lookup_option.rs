use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LookupOption {
    Local,
    Arbitrary,
    Optimistic,
    Conservative,
}

impl fmt::Display for LookupOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            LookupOption::Local => "Local",
            LookupOption::Arbitrary => "Arbitrary",
            LookupOption::Optimistic => "Optimistic",
            LookupOption::Conservative => "Conservative",
        })
    }
}
