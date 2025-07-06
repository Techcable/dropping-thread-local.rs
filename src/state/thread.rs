//! Thread-specific state.

use crate::state::GLOBAL_STATE;
use crate::state::ids::{LiveLocalId, LiveThreadId, UniqueThreadId};
use crate::state::local_defs::ActiveLocalDef;
use parking_lot::{Mutex, RwLockReadGuard, RwLockWriteGuard};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};

/// The state of a currently live thread.
pub struct ActiveThreadState {
    unique_id: UniqueThreadId,
    live_id: LiveThreadId,
    /// The actual values associated with this thread local.
    ///
    /// As of v0.2 [`boxcar::Vec`] is essentially structured the same as the internals
    /// of the `thread_local` crate.
    /// This means the performance characteristics of both should be similar.
    values: boxcar::Vec<LocalValueSlot>,
    /// Lock held to initialize any new value slots.
    ///
    /// Without this lock, `values` may only be read.
    value_init_lock: Mutex<()>,
}
impl ActiveThreadState {
    #[inline]
    pub fn unique_id(&self) -> UniqueThreadId {
        self.unique_id
    }
    #[inline]
    pub fn live_id(&self) -> LiveThreadId {
        self.live_id
    }
    /// Initialize the current thread state.
    ///
    /// ## Safety
    /// This function should be called once and only once for each thread.
    unsafe fn init() -> Arc<ActiveThreadState> {
        // inconsistent state here would be bad
        std::panic::abort_unwind(|| {
            let mut lock = GLOBAL_STATE.write();
            let (thread_uid, thread_index) = lock.id_manager.acquire_thread_id();
            let state = Arc::new(ActiveThreadState {
                unique_id: thread_uid,
                live_id: thread_index,
                values: boxcar::Vec::new(),
                value_init_lock: Mutex::new(()),
            });
            // SAFETY: IdManager + lock ensures not initialized twice
            unsafe { lock.register_thread(&state) };
            state
        })
    }
    #[inline]
    pub fn get_slot(&self, live_local_id: LiveLocalId) -> Option<&LocalValueSlot> {
        self.values.get(live_local_id.index())
    }
    #[inline]
    pub fn get_or_init_slot(&self, live_local: &Arc<ActiveLocalDef>) -> &LocalValueSlot {
        match self.get_slot(live_local.live_id()) {
            Some(existing) => existing,
            None => self.init_slot(live_local),
        }
    }
    #[cold]
    #[inline(never)]
    fn init_slot(&self, live_local: &Arc<ActiveLocalDef>) -> &LocalValueSlot {
        let _guard = self.value_init_lock.lock();
        let original_length = self.values.count();
        if live_local.live_id().index() >= original_length {
            loop {
                let new_index = self.values.push(LocalValueSlot::uninit());
                if new_index >= live_local.live_id().index() {
                    break;
                }
                self.values.push(LocalValueSlot::uninit());
            }
        }
        let slot = self
            .values
            .get(original_length)
            .expect("despite holding lock, value slot partially initialized");
        slot.local_def.store(
            Arc::into_raw(Arc::clone(&live_local)).cast_mut(),
            Ordering::Release,
        );
        slot
    }
}
impl Drop for ActiveThreadState {
    fn drop(&mut self) {
        // unwinds trigger aborts, consistent with the behavior of thread_local!
        std::panic::abort_unwind(|| {
            // need lock to unregister thread
            // each  slots contain a reference to the defining local,
            // so the active definitions are free to change once the thread is unregistered
            {
                let mut lock = GLOBAL_STATE.lock();
                // remove from public registry
                lock.unregister_thead(self);
                // Now free the id before running the destructors
                // SAFETY: Running drop guarantees the Arc is dead
                unsafe { lock.id_manager.release_live_thread_id(self.live_id) }
            }
            for mut value in self.values.into_iter() {
                let local_def = *value.local_def.get_mut();
                if local_def.is_null() {
                    continue;
                }
                // SAFETY: guaranteed to either represent a valid Arc or be null
                let local_def: Arc<ActiveLocalDef> = unsafe { Arc::from_raw(local_def) };
                let value_ptr = *value.value_ptr.get_mut();
                if value_ptr.is_null() {
                    continue; // null ptr - nothing to free
                }
                // SAFETY: Guaranteed to be either null or valid
                unsafe { local_def.drop_value_func()(value_ptr) }
            }
        });
    }
}

/// Holds a thread-local value.
///
/// ## Safety
/// Each field represents an owned value,
/// with its own access rules.
pub struct LocalValueSlot {
    /// The local definition, or null if the slot is not initialized.
    ///
    /// This is actually an owned `Arc`,
    /// which needs to be freed.
    pub(crate) local_def: AtomicPtr<ActiveLocalDef>,
    /// A pointer to the value.
    ///
    /// This is only safe to access if both the thread and local definitions
    /// are known to be valid and not being inappropriately modified.
    pub(crate) value_ptr: AtomicPtr<c_void>,
}
impl LocalValueSlot {
    const fn uninit() -> Self {
        LocalValueSlot {
            local_def: AtomicPtr::new(std::ptr::null_mut()),
            value_ptr: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
}

// this allows for fast access and avoids initialization checks
#[thread_local]
static STATE_PTR: Cell<Option<NonNull<ActiveThreadState>>> = const { Cell::new(None) };
// this is what calls the destructor on thread death
// needs to be an Arc in case there are concurrent accesses
thread_local! {
    static STATE: Arc<ActiveThreadState> = unsafe { ActiveThreadState::init() };
}
