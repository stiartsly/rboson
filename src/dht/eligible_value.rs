
use crate::{Id, Value};

#[derive(Clone, Default)]
pub(crate) struct EligibleValue {
    target: Id,
    expected_sequence_number: i32,
    value: Option<Value>,
}

impl EligibleValue {
    pub(crate) fn new(target: Id, expected_sequence_number: i32) -> Self {
        Self {
            target,
            expected_sequence_number,
            value: None,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    pub(crate) fn update(&mut self, value: Value, need_update: bool) -> bool {
        if value.id() != self.target
            || (self.expected_sequence_number >= 0
                && value.sequence_number() < self.expected_sequence_number)
            || !value.is_valid()
        {
            return false;
        }

        match self.value.as_ref() {
            Some(current) if value.sequence_number() <= current.sequence_number() => {}
            _ => self.value = Some(value.clone()),
        }

        true
    }

    pub(crate) fn needs_update(&self) -> bool {
        false
    }

    pub(crate) fn value(&self) -> Option<Value> {
        self.value.as_ref().map(|v|v.clone())
    }
}