use std::collections::BTreeSet;
use std::ffi::c_void;
use std::sync::Arc;

use foldhash::HashSet;
use parking_lot::{Mutex, RwLock};

use crate::state::GLOBAL_STATE;
use crate::state::ids::{LiveLocalId, LiveThreadId, UniqueLocalId};

pub struct ActiveLocalDef {
    unique_id: UniqueLocalId,
    live_id: LiveLocalId,
    drop_value_func: unsafe fn(*mut c_void),
    /// A list of threads which currently have initialized data.
    ///
    /// Needed for efficient iteration over live values,
    /// including for Drop.
    ///
    /// Uses BTreeSet because it is more memory efficient than HashSet.
    ///
    /// TODO: Need a Arc reference  that somehow preserves
    /// the thread id to make sure we are .
    /// Alternatively, use a HashMap<ThreadId, >
    /// The double use of "live" ids is somewhat necessary.
    initialized_threads: Mutex<BTreeSet<LiveThreadId>>,
}
impl ActiveLocalDef {
    #[inline]
    pub fn drop_value_func(&self) -> unsafe fn(*mut c_void) {
        self.drop_value_func
    }
    #[inline]
    pub fn unique_id(&self) -> UniqueLocalId {
        self.unique_id
    }
    #[inline]
    pub fn live_id(&self) -> LiveLocalId {
        self.live_id
    }
    /// Destroy the thread local definition.
    ///
    /// This logic cannot be the destructor,
    /// since each thread local slot has a strong reference to the definition
    /// which would prevent the Arc from being destroyed.
    pub fn destroy(self: Arc<Self>) {}
}
impl Drop for ActiveLocalDef {
    fn drop(&mut self) {
        // finally free the id, after we know there are no more uses
        let mut lock = GLOBAL_STATE.lock();
        // SAFETY: No uses of the definition, because all references have been freed
        unsafe {
            lock.id_manager.release_live_local_id(self.live_id);
        }
    }
}
