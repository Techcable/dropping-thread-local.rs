#![feature(
    thread_local, // faster than macro
    abort_unwind,
)]
use std::fmt::Debug;
use std::sync::Arc;

mod state;

/// Indicates a type can be quickly cloned.
///
/// ## Safety
/// While a slow clone can not technically cause memory safety issues,
/// it may cause the global thread-local lock to be held for an excessively long time.
/// Although not unsafe, triggering blocking in local-initialization
/// and thread destructors  is highly unexpected.
pub trait FastClone: Clone {}
impl<T: Copy> FastClone for T {}
impl<T> FastClone for Arc<T> {}
