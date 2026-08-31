use std::{
    collections::HashMap,
    marker::PhantomData,
};

pub(crate) struct ConnectionRegistry<T> {
    connections : HashMap<i32, bool>,
    inflight    : usize,
    phantom     : PhantomData<T>,
}

impl<T> ConnectionRegistry<T> {
    pub(crate) fn new() -> Self {
        Self {
            connections: HashMap::new(),
            inflight: 0,
            phantom: PhantomData,
        }
    }

    // Registers a newly established connection in the idle state.
    pub(crate) fn add(&mut self, connection: i32) {
        self.connections.insert(connection, false);
    }

    // Returns true if the removed connection was busy.
    pub(crate) fn remove(&mut self, connection: i32) -> bool {
        if self.connections.remove(&connection) == Some(true) {
            self.inflight -= 1;
            return true;
        }
        false
    }

    pub(crate) fn size(&self) -> usize {
        self.connections.len()
    }

    pub(crate) fn in_flight(&self) -> usize {
        self.inflight
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    // Returns true only on an idle -> busy transition.
    pub(crate) fn mark_busy(&mut self, connection: i32) -> bool {
        if let Some(state) = self.connections.get_mut(&connection) {
            if !*state {
                *state = true;
                self.inflight += 1;
                return true;
            }
        }
        false
    }

    // Returns true only on a busy -> idle transition.
    pub(crate) fn mark_idle(&mut self, connection: i32) -> bool {
        if let Some(state) = self.connections.get_mut(&connection) {
            if *state {
                *state = false;
                self.inflight -= 1;
                return true;
            }
        }
        false
    }

    pub(crate) fn connections(&self) -> Vec<i32> {
        self.connections.keys().copied().collect()
    }

    pub(crate) fn clear(&mut self) {
        self.connections.clear();
        self.inflight = 0;
    }
}
