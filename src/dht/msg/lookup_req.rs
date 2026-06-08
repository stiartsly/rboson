use crate::Id;

pub(crate) const WANT4_MASK: i32 = 0x01;
pub(crate) const WANT6_MASK: i32 = 0x02;
pub(crate) const WANT_TOKEN_MASK: i32 = 0x04;

#[derive(Clone)]
pub(crate) struct Data {
    target  : Id,
    want4   : bool,
    want6   : bool,
    want_token: bool,
}

impl Data {
    pub(crate) fn new(
        target: Id,
        want4: bool,
        want6: bool,
        want_token: bool
    ) -> Self {
        Self {target, want4, want6, want_token}
    }
}

pub(crate) trait LookupRequest {
    fn data(&self) -> &Data;

    fn target(&self) -> &Id {
        &self.data().target
    }

    fn want4(&self) -> bool {
        self.data().want4
    }

    fn want6(&self) -> bool {
        self.data().want6
    }

    fn want_token(&self) -> bool {
        self.data().want_token
    }

    fn want(&self) -> i32 {
        (if self.want4() { 0x01 } else { 0x00 }) |
        (if self.want6() { 0x02 } else { 0x00 }) |
        (if self.want_token() { 0x04 } else { 0x00 })
    }
}
