//! Defines the [`GlobalState`], which is protected by a lock.
//!
//! This limits the actions that can be done concurrently.

use crate::state::ids::{IdManager, LiveLocalId, LiveThreadId, UniqueLocalId, UniqueThreadId};
use crate::state::local_defs::ActiveLocalDef;
use crate::state::thread::ActiveThreadState;
use parking_lot::{Mutex, RwLock};
use std::cell::RefCell;
use std::sync::{Arc, Weak};

/// The global state of the thread_locals,
/// protected by a single mutex.
///
/// The lock needs to be held to initialize/deinitialize a thread
/// or to define a new thread local.
/// It does not need to be held to access existing thread loals.
pub struct GlobalState {
    /// Manages the ids of the .
    pub(crate) id_manager: IdManager,
    active_thread_slots: Vec<ActiveThreadSlot>,
}
impl GlobalState {
    pub fn active_threads_iter(&self) -> impl Iterator<Item = Arc<ActiveThreadState>> + '_ {
        self.active_thread_slots.iter().filter_map(|slot| {
            let uid = slot.id?;
            let state = slot.state.upgrade()?;
            assert_eq!(state.unique_id(), uid, "inconsistent thread state");
            Some(state)
        })
    }
    pub fn active_local_slots(&self) -> usize {
        self.active_local_slots.len()
    }
    #[inline]
    pub unsafe fn get_thread_slot_unchecked(&self, live_id: LiveThreadId) -> &ActiveThreadSlot {
        self.active_thread_slots.get_unchecked(live_id.index())
    }

    #[inline]
    pub unsafe fn get_local_slot_unchecked(&self, live_id: LiveLocalId) -> &ActiveLocalDefSlot {
        self.active_local_slots.get_unchecked(live_id.index())
    }

    /// Activate the specified thread.
    ///
    /// ## Safety
    /// Undefined behavior if the thread has already been initialized.
    pub(crate) unsafe fn register_thread(&mut self, state: &Arc<ActiveThreadState>) {
        while self.active_thread_slots.len() >= state.live_id().index() {
            self.active_thread_slots.push(ActiveThreadSlot::uninit());
        }
        let slot = &mut self.active_thread_slots[state.live_id().index()];
        assert_eq!(slot.id, None, "slot already associated with thread");
        slot.id = Some(state.unique_id());
        slot.state = Arc::downgrade(state);
    }
    pub fn unregister_thead(&mut self, state: &ActiveThreadState) {
        let slot = &mut self.active_thread_slots[state.live_id().index()];
        assert_eq!(
            slot.id.take(),
            Some(state.unique_id()),
            "unexpected thread id for slot {:?}",
            state.live_id().index(),
        );
    }
}
pub static GLOBAL_STATE: Mutex<GlobalState> = const {
    Mutex::new(GlobalState {
        id_manager: IdManager::new(),
        active_thread_slots: Vec::new(),
    })
};
struct ActiveThreadSlot {
    pub id: Option<UniqueThreadId>,
    pub state: Weak<ActiveThreadState>,
}
impl ActiveThreadSlot {
    pub const fn uninit() -> Self {
        ActiveThreadSlot {
            id: None,
            state: Weak::new(),
        }
    }
}
