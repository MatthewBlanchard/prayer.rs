use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Single synchronization boundary for a canonical shared-world snapshot.
pub struct KnowledgeStore<State> {
    state: RwLock<Arc<State>>,
}

impl<State> KnowledgeStore<State> {
    pub fn new(state: State) -> Self {
        Self {
            state: RwLock::new(Arc::new(state)),
        }
    }

    pub fn read(&self) -> KnowledgeReadGuard<'_, State> {
        KnowledgeReadGuard(self.state.read())
    }

    pub fn snapshot(&self) -> Arc<State> {
        Arc::clone(&self.state.read())
    }
}

impl<State: Clone> KnowledgeStore<State> {
    pub fn write(&self) -> KnowledgeWriteGuard<'_, State> {
        KnowledgeWriteGuard(self.state.write())
    }
}

pub struct KnowledgeReadGuard<'a, State>(RwLockReadGuard<'a, Arc<State>>);

impl<State> Deref for KnowledgeReadGuard<'_, State> {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

pub struct KnowledgeWriteGuard<'a, State: Clone>(RwLockWriteGuard<'a, Arc<State>>);

impl<State: Clone> Deref for KnowledgeWriteGuard<'_, State> {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<State: Clone> DerefMut for KnowledgeWriteGuard<'_, State> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_reuse_the_canonical_allocation_until_a_write() {
        let store = KnowledgeStore::new(vec![1, 2]);
        let first = store.snapshot();
        let second = store.snapshot();

        assert!(Arc::ptr_eq(&first, &second));

        store.write().push(3);
        let third = store.snapshot();

        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(first.as_slice(), &[1, 2]);
        assert_eq!(third.as_slice(), &[1, 2, 3]);
    }
}
