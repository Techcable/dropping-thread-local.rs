use alloc::sync::Arc;
use core::fmt::{Debug, Formatter};
use core::ops::Deref;

use intid_allocator::IdAllocator;
use nonmax::NonMaxUsize;
use parking_lot::Mutex;

static RAW_ID_ALLOCATOR: Mutex<IdAllocator<LiveLocalId>> = Mutex::new(IdAllocator::new());

struct LocalIdInner {
    id: LiveLocalId,
}
impl Drop for LocalIdInner {
    fn drop(&mut self) {
        RAW_ID_ALLOCATOR.lock().free(self.id);
    }
}
/// Owned reference to a local ID.
///
/// Once all of these owned references have been dropped,
/// the ID can be reused.
#[derive(Clone)]
pub struct OwnedLocalId {
    _inner: Arc<LocalIdInner>,
    id: LiveLocalId,
}
impl OwnedLocalId {
    /// Allocate a new id for a
    pub fn alloc() -> OwnedLocalId {
        let id = RAW_ID_ALLOCATOR.lock().alloc();
        OwnedLocalId {
            id,
            _inner: Arc::new(LocalIdInner { id }),
        }
    }

    /// Get the id that is owned.
    #[inline]
    pub fn id(&self) -> LiveLocalId {
        self.id
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.id.0.get()
    }
}
impl Debug for OwnedLocalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OwnedLocalId")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
impl Deref for OwnedLocalId {
    type Target = LiveLocalId;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.id
    }
}
/// An integer index corresponding to a live ID.
///
/// There is nothing keeping this id valid,
/// so it could be reused by another local while still in use.
/// To prevent this, use [`OwnedLocalId`].
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq, Hash)]
pub struct LiveLocalId(NonMaxUsize);
impl intid::IntegerId for LiveLocalId {
    type Int = usize;

    #[inline]
    fn from_int_checked(id: Self::Int) -> Option<Self> {
        Some(LiveLocalId(NonMaxUsize::new(id)?))
    }

    #[inline]
    fn to_int(self) -> Self::Int {
        self.0.get()
    }
}
impl intid::ContiguousIntegerId for LiveLocalId {
    const MIN_ID: Self = LiveLocalId(NonMaxUsize::ZERO);
    const MAX_ID: Self = LiveLocalId(NonMaxUsize::MAX);
}
impl intid::IntegerIdCounter for LiveLocalId {
    const START: Self = LiveLocalId(NonMaxUsize::ZERO);
    const START_INT: Self::Int = 0;
}
