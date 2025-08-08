#![deny(
    // currently there is no unsafe code
    unsafe_code,
    // public library should have docs
    missing_docs,
)]
//! Dynamically allocated thread locals that properly run destructors when a thread is destroyed.
//!
//! This is in contrast to the [`thread_local`] crate, which has similar functionality,
//! but only runs destructors when the `ThreadLocal` object is dropped.
//! This crate guarantees that one thread will never see the thread-local data of another,
//! which can happen in the `thread_local` crate due to internal storage reuse.
//!
//! This crate attempts to implement "true" thread locals,
//! mirroring [`std::thread_local!`] as closely as possible.
//! I would say the `thread_local` crate is good for functionality like reusing allocations
//! or for having local caches that can be sensibly reused once a thread dies.
//!
//! This crate will attempt to run destructors as promptly as possible,
//! but taking snapshots may interfere with this (see below).
//! Panics in thread destructors will cause aborts, just like they do with [`std::thread_local!`].
//!
//! Right now, this crate has no unsafe code.
//! This may change if it can bring a significant performance improvement.
//!
//! # Snapshots
//! The most complicated feature of this library is snapshots.
//! It allows anyone who has access to a [`DroppingThreadLocal`] to iterate over all currently live
//! values using the [`DroppingThreadLocal::snapshot_iter`] method.
//!
//! This will return a snapshot of the live values at the time the method is called,
//! although if a thread dies during iteration, it may not show up.
//! See the method documentation for more details.
//!
//! # Performance
//! Lookup is based around a hashmap, and I expect the current implementation to be noticeably slower than either [`std::thread_local!`] or the [`thread_local`] crate.
//! The former is a low-cost abstraction over native thread local storage and the latter is written by the author of`hashbrown` and `parking_lot`.
//!
//! A very basic benchmark on my M1 Mac and Linux Laptop (`i5-7200U` circa 2017) gives the following results:
//!
//! | library                 | does `Arc::clone` | time (M1 Mac) | time (i5 ~2017 Laptop)    |
//! |-------------------------|-------------------|---------------|---------------------------|
//! | `std`                   | no                |  0.42 ns      |  0.69 ns                  |
//! | `std`                   | *yes*             | 11.49 ns      | 14.01 ns                  |
//! | `thread_local`          | no                |  1.38 ns      |  1.38 ns                  |
//! | `thread_local`          | *yes*             | 11.43 ns      | 14.02 ns                  |
//! | `dropping_thread_local` | *yes*             | 13.14 ns      | 31.14 ns                  |
//!
//! Every lookup in the current implementation of `dropping_thread_local` requires calling `Arc::clone`.
//! This has significant overhead in its own right, so I benchmarked the other libraries both storing their data an regular `Box` vs. storing data in an `Arc` and doing `Arc::clone`.
//!
//! On my Mac, the library ranges between 30% slower than calling `thread_local::ThreadLocal::get` + `Arc::clone` and 30x slower than a plain `std::thread_local!`. On my older Linux laptop, this library ranges between 3x slower than `thread_local::ThreadLocal::get` + `Arc::clone` and 60x slower than a plain `std::thread_local`.
//!
//! This performance is a lot better than I expected (at least on the macbook). I am also disappointed by the performance of `Arc::clone`. Further improvements beyond this will almost certainly require amount of `unsafe` code. I have three ideas for improvement:
//!
//! - Avoid requiring `Arc::clone` by using a [`LocalKey::with`] style API, and making `drop(DroppingThreadLocal)` delay freeing values from live threads until after that live thread dies.
//! - Use [biased reference counting] instead of an `Arc`. This would not require `unsafe` code directly in this crate. Unfortunately, biased reference counting can delay destruction or even leak if the heartbeat function is not called. The [`trc` crate] will not work, as `trc::Trc` is `!Send`.
//! - Using [`boxcar::Vec`] instead of a `HashMap` for lookup. This is essentially the same data structure that the `thread_local` crate uses, so should make the lookup performance similar.
//!
//! [biased reference counting]: https://dl.acm.org/doi/10.1145/3243176.3243195
//! [`LocalKey::with`]: https://doc.rust-lang.org/std/thread/struct.LocalKey.html#method.with
//! [`boxcar::Vec`]: https://docs.rs/boxcar/0.2.13/boxcar/struct.Vec.html
//! [`trc` crate]: https://github.com/ericlbuehler/trc
//!
//! ## Locking
//! The implementation needs to acquire a global lock to initialize/deinitialize threads and create new locals.
//! Accessing thread-local data is also protected by a per-thread lock.
//! This lock should be uncontended, and [`parking_lot::Mutex`] should make this relatively fast.
//! I have been careful to make sure that locks are not held while user code is being executed.
//! This includes releasing locks before any destructors are executed.
//!
//! # Limitations
//! The type that is stored must be `Send + Sync + 'static`.
//! The `Send` bound is necessary because the [`DroppingThreadLocal`] may be dropped from any thread.
//! The `Sync` bound is necessary to support snapshots,
//! and the `'static` bound is due to internal implementation chooses (use of safe code).
//!
//! A Mutex can be used to work around the `Sync` limitation.
//! (I recommend [`parking_lot::Mutex`], which is optimized for uncontented locks)
//! You can attempt to use the [`fragile`] crate to work around the `Send` limitation,
//! but this will cause panics if the value is dropped from another thead.
//! Some ways a value can be dropped from another thread if a snapshot keeps the value alive,
//! or if the [`DroppingThreadLocal`] itself is dropped.
//!
//! [`thread_local`]: https://docs.rs/thread_local/1.1/thread_local/
//! [`fragile`]: https://docs.rs/fragile/2/fragile/

extern crate alloc;
extern crate core;

use alloc::rc::Rc;
use alloc::sync::{Arc, Weak};
use core::any::Any;
use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZero;
use core::ops::Deref;
use core::sync::atomic::Ordering;
use std::thread::ThreadId;

use parking_lot::Mutex;
use portable_atomic::AtomicU64;

/// A thread local that drops its value when the thread is destroyed.
///
/// See module-level documentation for more details.
///
/// Dropping this value will free all the associated values.
pub struct DroppingThreadLocal<T: Send + Sync + 'static> {
    id: UniqueLocalId,
    marker: PhantomData<Arc<T>>,
}
impl<T: Send + Sync + 'static + Debug> Debug for DroppingThreadLocal<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let value = self.get();
        f.debug_struct("DroppingThreadLocal")
            .field("local_data", &value.as_ref().map(|value| value.as_ref()))
            .finish()
    }
}
impl<T: Send + Sync + 'static> Default for DroppingThreadLocal<T> {
    #[inline]
    fn default() -> Self {
        DroppingThreadLocal::new()
    }
}
impl<T: Send + Sync + 'static> DroppingThreadLocal<T> {
    /// Create a new thread-local value.
    #[inline]
    pub fn new() -> Self {
        DroppingThreadLocal {
            id: UniqueLocalId::alloc(),
            marker: PhantomData,
        }
    }
    /// Get the value associated with the current thread,
    /// or `None` if not initialized.
    #[inline]
    pub fn get(&self) -> Option<SharedRef<T>> {
        THREAD_STATE.with(|thread| {
            Some(SharedRef {
                thread_id: thread.id,
                value: thread.get(self.id)?.downcast::<T>().expect("unexpected type"),
            })
        })
    }
    /// Get the value associated with the current thread,
    /// initializing it if not yet defined.
    #[inline]
    pub fn get_or_init(&self, func: impl FnOnce() -> T) -> SharedRef<T> {
        THREAD_STATE.with(|thread| {
            let value = thread
                .get(self.id)
                .unwrap_or_else(|| {
                    let mut func = Some(func);
                    thread.init(self.id, &mut || {
                        let func = func.take().unwrap();
                        Arc::new(func()) as Arc<dyn Any + Send + Sync + 'static>
                    })
                })
                .downcast::<T>()
                .expect("unexpected type");
            SharedRef {
                thread_id: thread.id,
                value,
            }
        })
    }
    /// Iterate over currently live values and their associated thread ids.
    ///
    /// New threads that have been spanned after the snapshot was taken will not be present
    /// in the iterator.
    /// Threads that die after the snapshot is taken may or may not be present.
    /// Values from threads that die before the snapshot will not be present.
    ///
    /// The order of the iteration is undefined.
    pub fn snapshot_iter(&self) -> SnapshotIter<T> {
        let Some(snapshot) = snapshot_live_threads() else {
            return SnapshotIter {
                local_id: self.id,
                iter: None,
                marker: PhantomData,
            };
        };
        SnapshotIter {
            local_id: self.id,
            iter: Some(snapshot.into_iter()),
            marker: PhantomData,
        }
    }
}
impl<T: Send + Sync + 'static> Drop for DroppingThreadLocal<T> {
    fn drop(&mut self) {
        // want to drop without holding the lock
        let Some(snapshot) = snapshot_live_threads() else {
            // no live threads -> nothing to free
            return;
        };
        // panics won't cause aborts here, there is no need for them
        for (thread_id, thread) in snapshot {
            if let Some(thread) = Weak::upgrade(&thread) {
                assert_eq!(thread.id, thread_id);
                let value: Option<DynArc> = {
                    let mut lock = thread.values.lock();
                    lock.remove(&self.id)
                };
                // drop value once lock no longer held
                drop(value);
            }
        }
    }
}
/// Iterates over a snapshot of the values,
/// given by [`DroppingThreadLocal::snapshot_iter`].`
///
/// Due to thread death, it is not possible to know the exact size of the iterator.
pub struct SnapshotIter<T: Send + Sync + 'static> {
    local_id: UniqueLocalId,
    iter: Option<imbl::hashmap::ConsumingIter<(ThreadId, Weak<LiveThreadState>), imbl::shared_ptr::DefaultSharedPtr>>,
    // do not make Send+Sync, for flexibility in the future
    marker: PhantomData<Rc<T>>,
}
impl<T: Send + Sync + 'static> Iterator for SnapshotIter<T> {
    type Item = SharedRef<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // None from either of these means we have nothing left to iterate
            let (thread_id, thread) = self.iter.as_mut()?.next()?;
            let Some(thread) = Weak::upgrade(&thread) else { continue };
            let Some(arc) = ({
                let lock = thread.values.lock();
                lock.get(&self.local_id).cloned()
            }) else {
                continue;
            };
            return Some(SharedRef {
                thread_id,
                value: arc.downcast::<T>().expect("mismatched type"),
            });
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.iter {
            Some(ref iter) => {
                // may be zero if all threads die
                (0, Some(iter.len()))
            }
            None => (0, Some(0)),
        }
    }
}
impl<T: Send + Sync + 'static> core::iter::FusedIterator for SnapshotIter<T> {}

/// A shared reference to a thread local value.
///
/// This may be cloned and sent across threads.
/// May delay destruction of value past thread death.
#[derive(Clone, Debug)]
pub struct SharedRef<T> {
    thread_id: ThreadId,
    value: Arc<T>,
}
impl<T> SharedRef<T> {
    /// The thread id the value was
    #[inline]
    pub fn thread_id(this: &Self) -> ThreadId {
        this.thread_id
    }
}

impl<T> Deref for SharedRef<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> AsRef<T> for SharedRef<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.value
    }
}
impl<T: Hash> Hash for SharedRef<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}
impl<T: Eq> Eq for SharedRef<T> {}
impl<T: PartialEq> PartialEq for SharedRef<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl<T: PartialOrd> PartialOrd for SharedRef<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        T::partial_cmp(&self.value, &other.value)
    }
}
impl<T: Ord> Ord for SharedRef<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        T::cmp(&self.value, &other.value)
    }
}

struct UniqueIdAllocator {
    next_id: AtomicU64,
}
impl UniqueIdAllocator {
    const fn new() -> Self {
        UniqueIdAllocator {
            next_id: AtomicU64::new(1),
        }
    }
    fn alloc(&self) -> NonZero<u64> {
        NonZero::new(
            self.next_id
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |x| x.checked_add(1))
                .expect("id overflow"),
        )
        .unwrap()
    }
}
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq, Hash)]
struct UniqueLocalId(NonZero<u64>);
impl UniqueLocalId {
    fn alloc() -> Self {
        static ALLOCATOR: UniqueIdAllocator = UniqueIdAllocator::new();
        UniqueLocalId(ALLOCATOR.alloc())
    }
}

type LiveThreadMap = imbl::GenericHashMap<
    ThreadId,
    Weak<LiveThreadState>,
    foldhash::fast::RandomState,
    imbl::shared_ptr::DefaultSharedPtr,
>;
/// Map of currently live threads.
///
/// This is a persistent map to allow quick snapshots to be taken by drop function and iteration/
/// I use `imbl` instead of `rpds`, because `rpds` doesn't support an owned iterator,
/// only a borrowed iterator which would require an extra allocation.
/// I use a hashmap instead of a `BTreeMap` because that would require `ThreadId: Ord`,
/// which the stdlib doesn't have.
static LIVE_THREADS: Mutex<Option<LiveThreadMap>> = Mutex::new(None);
fn snapshot_live_threads() -> Option<LiveThreadMap> {
    let lock = LIVE_THREADS.lock();
    lock.as_ref().cloned()
}

thread_local! {
    static THREAD_STATE: Arc<LiveThreadState> = {
        let id = std::thread::current().id();
        let state =Arc::new(LiveThreadState {
            id,
            values: Mutex::new(foldhash::HashMap::default()),
        });
        let mut live_threads = LIVE_THREADS.lock();
        let live_threads = live_threads.get_or_insert_default();
        use imbl::hashmap::Entry;
        match live_threads.entry(id) {
            Entry::Occupied(_) => panic!("reinitialized thread"),
            Entry::Vacant(entry) => {
                entry.insert(Arc::downgrade(&state));
            }
        }
        state
    };
}
type DynArc = Arc<dyn Any + Send + Sync + 'static>;
struct LiveThreadState {
    id: ThreadId,
    /// Maps from local ids to values.
    ///
    /// ## Performance
    /// This is noticeably slower than what `thread_local`.
    ///
    /// We could make it faster if we used a vector
    /// A `boxcar::Vec` is essentially the same data structure as `thread_local` uses,
    /// and would allow us to get rid of the lock.
    /// The only difference is we would be mapping from local ids -> values,
    /// However, the lock is uncontented and parking_lot makes that relatively cheap,
    /// so a `Mutex<Vec<T>> might also work and be simpler.
    /// To avoid unbounded memory usage if locals are constantly being allocated/drops,
    /// this would require reusing indexes.
    values: Mutex<foldhash::HashMap<UniqueLocalId, DynArc>>,
}
impl LiveThreadState {
    #[inline]
    fn get(&self, id: UniqueLocalId) -> Option<DynArc> {
        let lock = self.values.lock();
        Some(Arc::clone(lock.get(&id)?))
    }
    // the arc has dynamic type to avoid monomorphization
    #[cold]
    #[inline(never)]
    fn init(&self, id: UniqueLocalId, init: &mut dyn FnMut() -> DynArc) -> DynArc {
        let new_value = init();
        let mut lock = self.values.lock();
        use std::collections::hash_map::Entry;
        match lock.entry(id) {
            Entry::Occupied(_) => {
                panic!("unexpected double initialization of thread-local value")
            }
            Entry::Vacant(entry) => {
                entry.insert(Arc::clone(&new_value));
            }
        }
        new_value
    }
}
impl Drop for LiveThreadState {
    fn drop(&mut self) {
        // clear all our values
        self.values.get_mut().clear();
        // remove from the list of live threads
        {
            let mut threads = LIVE_THREADS.lock();
            if let Some(threads) = threads.as_mut() {
                // no fear of dropping while locked because we control the type
                let state: Option<Weak<LiveThreadState>> = threads.remove(&self.id);
                drop(state)
            }
        }
    }
}
