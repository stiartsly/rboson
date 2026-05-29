
use crate::{Id, Value};

pub(crate) struct EligibleValue {
    target  : Id,
    expected_seq: i32,
    value   : Option<Value>,
    latest  : bool,
}

impl EligibleValue {
    pub(crate) fn new(target: Id, expected_seq: i32) -> Self {
        Self {
            target,
            expected_seq,
            value   : None,
            latest  : false
        }
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    pub(crate) fn update(&mut self, value: Value, latest: bool) -> bool {
        if value.id() != self.target
            || (self.expected_seq >= 0 && value.sequence_number() < self.expected_seq)
            || !value.is_valid()
        {
            return false;
        }

        if self.value.as_ref().map_or(true, |v| value.sequence_number() > v.sequence_number()) {
            self.value = Some(value);
            self.latest = latest;
        }
        true
    }

    pub(crate) fn is_latest(&self) -> bool {
        self.latest
    }

    pub(crate) fn value(&mut self) -> Option<Value> {
        self.value.take()
    }
}
