use portable_atomic::AtomicU64;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};
use std::num::NonZero;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

/// Manages thread and local ids.
///
/// Allocating and freeing ids needs exclusive `&mut` access to the [`GlobalState`].
///
/// [`GlobalState`]: crate::state::GlobalState
pub struct IdManager {
    thread_ids: ReusingIdManager,
    local_ids: ReusingIdManager,
    /// Lazy initialized set of seen thread ids,
    /// to protect against double-initialization.
    ///
    /// Uses BTreeSet for memory efficiency.
    seen_threads: Option<BTreeSet<UniqueThreadId>>,
}
impl IdManager {
    pub const fn new() -> Self {
        IdManager {
            thread_ids: ReusingIdManager::new(),
            local_ids: ReusingIdManager::new(),
            seen_threads: None,
        }
    }
    pub fn acquire_local_id(&mut self) -> (UniqueLocalId, LiveLocalId) {
        (
            UniqueLocalId::acquire(),
            LiveLocalId(self.local_ids.get_mut().alloc()),
        )
    }
    /// Acquire a [`LiveThreadId`] for the current thread.
    ///
    /// ## Safety
    /// The currently executing thread id must be the one you are acquiring the id for.
    ///
    /// A single thread can only be associated with one `LiveThreadId` during the course of its lifetime.
    #[track_caller]
    pub fn acquire_thread_id(&mut self) -> (UniqueThreadId, LiveThreadId) {
        let uid = UniqueThreadId::current();
        {
            // i don't really trust thread initialization that much
            // since double init is UB, we should check for it
            let seen_threads = self.seen_threads.get_or_insert_default();
            let is_new = seen_threads.insert(uid);
            assert!(
                is_new,
                "Already acquired live id for {uid:?} (double initialization?)"
            );
        }
        (uid, LiveThreadId(self.thread_ids.get_mut().alloc()))
    }
    /// Release a live thread id, allowing for reuse by another thread.
    ///
    /// ## Safety
    /// After a thread's live id is freed, the thread must never allocate a new one.
    pub(crate) unsafe fn release_live_thread_id(&mut self, tid: LiveThreadId) {
        self.thread_ids.free(tid.0);
    }

    /// Release a live local def id, allowing for later reuse.
    ///
    /// ## Safety
    /// Undefined behavior to redefine a local unless all its data has been cleared.
    pub(crate) unsafe fn release_live_local_id(&mut self, tid: LiveLocalId) {
        self.local_ids.free(tid.0)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UniqueLocalId(NonZero<u64>);
impl UniqueLocalId {
    #[track_caller]
    #[cold]
    fn acquire() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        UniqueLocalId(
            NonZero::new(
                NEXT_ID
                    .fetch_update(Ordering::AcqRel, Ordering::AcqRel, |i| i.checked_add(1))
                    .ok()
                    .expect("ID overflow"),
            )
            .unwrap(),
        )
    }
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LiveLocalId(usize);
impl LiveLocalId {
    /// Create a live local id from an index.
    ///
    /// ## Safety
    /// Do not use this local id in a place where it is unexepxted.
    #[inline]
    pub unsafe fn from_index(index: usize) -> Self {
        LiveLocalId(index)
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.0
    }
}

/// Represents the id of a live thread.
///
/// Unlike a [`UniqueThreadId`], these can be reused.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LiveThreadId(usize);
impl LiveThreadId {
    #[inline]
    pub fn index(&self) -> usize {
        self.0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UniqueThreadId(std::thread::ThreadId);
impl UniqueThreadId {
    /// Get the id of the currently executing thread.
    #[inline]
    pub fn current() -> Self {
        UniqueThreadId(std::thread::current().id())
    }
}

/// Allocates integer ids, aggressively  reusing IDs that have been freed.
/// When ids are used to index items in a vector, this can save memory usage.
///
/// *CREDIT*: Implementation copied from [`thread_local::ThreadIdManager`] by @Amanieu.
/// That code is Apache-2.0 OR MIT licensed, so this should be fine.
/// [`thread_local::ThreadIdManager`]: https://github.com/Amanieu/thread_local-rs/blob/v1.1.9/src/thread_id.rs#L14-L17
struct ReusingIdManager {
    free_from: usize,
    free_list: Option<BinaryHeap<Reverse<usize>>>,
}

impl ReusingIdManager {
    pub const fn new() -> Self {
        Self {
            free_from: 0,
            free_list: None,
        }
    }

    #[cold]
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.free_list.as_mut().and_then(|heap| heap.pop()) {
            id.0
        } else {
            // `free_from` can't overflow as each thread takes up at least 2 bytes of memory and
            // thus we can't even have `usize::MAX / 2 + 1` threads.

            let id = self.free_from;
            self.free_from += 1;
            id
        }
    }

    #[cold]
    pub fn free(&mut self, id: usize) {
        self.free_list
            .get_or_insert_with(BinaryHeap::new)
            .push(Reverse(id));
    }
}
