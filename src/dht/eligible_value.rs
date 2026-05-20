
use crate::{Id, Value};

#[derive(Clone, Default)]
pub(crate) struct EligibleValue {
    target: Id,
    expected_seq: i32,
    value: Option<Value>,
    need_update: bool,
}

impl EligibleValue {
    pub(crate) fn new(target: Id, expected_seq: i32) -> Self {
        Self {
            target,
            expected_seq,
            value: None,
            need_update: false,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    pub(crate) fn update(&mut self, value: Value, need_update: bool) -> bool {
        if value.id() != self.target
            || (self.expected_seq >= 0 && value.sequence_number() < self.expected_seq)
            || !value.is_valid()
        {
            return false;
        }

        match self.value.as_ref() {
            Some(current) if value.sequence_number() <= current.sequence_number() => {}
            _ => {
                self.value = Some(value.clone());
                self.need_update = need_update;
            }
        }

        true
    }

    pub(crate) fn needs_update(&self) -> bool {
        self.need_update
    }

    pub(crate) fn value(&self) -> Option<Value> {
        self.value.as_ref().map(|v|v.clone())
    }
}
