use std::rc::Rc;
use ciborium::Value as CVal;

use crate::{
    unwrap,
    Id
};

pub(crate) struct Data {
    target: Option<Rc<Id>>,
    want4: bool,
    want6: bool,
    want_token: bool,
}

impl Data {
    pub(crate) fn new(want_token: bool) -> Self {
        Self {
            target: None,
            want4: false,
            want6: false,
            want_token,
        }
    }
}

pub(crate) trait Msg {
    fn data(&self) -> &Data;
    fn data_mut(&mut self) -> &mut Data;

    fn target(&self) -> Rc<Id> {
        unwrap!(self.data().target).clone()
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

    fn with_target(&mut self, target: Rc<Id>) {
        self.data_mut().target = Some(target)
    }

    fn with_want4(&mut self, want: bool) {
        self.data_mut().want4 = want
    }

    fn with_want6(&mut self, want: bool) {
        self.data_mut().want6 = want
    }

    fn with_want_token(&mut self, want: bool) {
        self.data_mut().want_token = want
    }

    fn to_cbor(&self) -> CVal {
        CVal::Map(vec![
            (
                CVal::Text(String::from("t")),
                self.target().to_cbor()
            ),
            (
                CVal::Text(String::from("w")),
                CVal::Integer(self.want().into())
            )
        ])
    }
}
