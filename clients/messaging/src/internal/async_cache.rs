use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard},
};

use boson_api::{Id, Result};

use super::MessagingRepository;

/// A write-through cache backed by the local repository.
///
/// Reads first check the in-memory map, then fall back to the caller-provided
/// repository loader. Writes update the map and immediately persist through the
/// caller-provided repository writer.
pub(crate) struct AsyncCache<V> {
    repository: Rc<RefCell<MessagingRepository>>,
    entries: Arc<Mutex<HashMap<Id, V>>>,
}

impl<V> AsyncCache<V> {
    pub(crate) fn new(repository: Rc<RefCell<MessagingRepository>>) -> Self {
        Self {
            repository,
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn shared(&self) -> Arc<Mutex<HashMap<Id, V>>> {
        self.entries.clone()
    }

    pub(crate) fn borrow(&self) -> CacheRef<'_, V> {
        CacheRef(self.entries.lock().unwrap())
    }

    pub(crate) fn borrow_mut(&self) -> CacheRef<'_, V> {
        CacheRef(self.entries.lock().unwrap())
    }

    pub(crate) fn lock(&self) -> std::result::Result<CacheRef<'_, V>, ()> {
        self.entries.lock().map(CacheRef).map_err(|_| ())
    }

    pub(crate) fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }

    pub(crate) fn remove(&self, id: &Id) -> Option<V> {
        self.entries.lock().unwrap().remove(id)
    }

    pub(crate) fn snapshot(&self) -> HashMap<Id, V>
    where
        V: Clone,
    {
        self.entries.lock().unwrap().clone()
    }
}

impl<V: Clone> AsyncCache<V> {
    /// Returns the cached value, loading and caching it from the repository on a miss.
    pub(crate) fn get<F>(&self, id: &Id, load: F) -> Result<Option<V>>
    where
        F: FnOnce(&mut MessagingRepository, &Id) -> Result<Option<V>>,
    {
        if let Some(value) = self.entries.lock().unwrap().get(id).cloned() {
            return Ok(Some(value));
        }
        let Some(value) = load(&mut self.repository.borrow_mut(), id)? else {
            return Ok(None);
        };
        self.entries
            .lock()
            .unwrap()
            .insert(*id, value.clone());
        Ok(Some(value))
    }

    /// Stores a value in memory and persists it through the repository writer.
    pub(crate) fn put<F>(&self, id: Id, value: V, save: F) -> Result<()>
    where
        F: FnOnce(&mut MessagingRepository, &V) -> Result<()>,
    {
        self.entries.lock().unwrap().insert(id, value.clone());
        save(&mut self.repository.borrow_mut(), &value)
    }
}

pub(crate) struct CacheRef<'a, V>(MutexGuard<'a, HashMap<Id, V>>);

impl<V> Deref for CacheRef<'_, V> {
    type Target = HashMap<Id, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V> DerefMut for CacheRef<'_, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
