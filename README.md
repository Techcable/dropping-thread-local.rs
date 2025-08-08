# dropping-thread-local [![Latest Version]][crates.io]

[Latest Version]: https://img.shields.io/crates/v/dropping-thread-local.svg
[crates.io]: https://crates.io/crates/dropping-thread-local
<!-- cargo-rdme start -->

Dynamically allocated thread locals that properly run destructors when a thread is destroyed.

This is in contrast to the [`thread_local`] crate, which has similar functionality,
but only runs destructors when the `ThreadLocal` object is dropped.
This crate guarantees that one thread will never see the thread-local data of another,
which can happen in the `thread_local` crate due to internal storage reuse.

This crate attempts to implement "true" thread locals,
mirroring [`std::thread_local!`] as closely as possible.
I would say the `thread_local` crate is good for functionality like reusing allocations
or for having local caches that can be sensibly reused once a thread dies.

This crate will attempt to run destructors as promptly as possible,
but taking snapshots may interfere with this (see below).
Panics in thread destructors will cause aborts, just like they do with [`std::thread_local!`].

Right now, this crate has no unsafe code.
This may change if it can bring a significant performance improvement.

## Snapshots
The most complicated feature of this library is snapshots.
It allows anyone who has access to a [`DroppingThreadLocal`] to iterate over all currently live
values using the [`DroppingThreadLocal::snapshot_iter`] method.

This will return a snapshot of the live values at the time the method is called,
although if a thread dies during iteration, it may not show up.
See the method documentation for more details.

## Performance
Lookup is based around a hashmap, and I expect the current implementation to be noticeably slower than either [`std::thread_local!`] or the [`thread_local`] crate.
The former is a low-cost abstraction over native thread local storage and the latter is written by the author of`hashbrown` and `parking_lot`.

A very basic benchmark on my M1 Mac and Linux Laptop (`i5-7200U` circa 2017) gives the following results:

| library                 | does `Arc::clone` | time (M1 Mac) | time (i5 ~2017 Laptop)    |
|-------------------------|-------------------|---------------|---------------------------|
| `std`                   | no                |  0.42 ns      |  0.69 ns                  |
| `std`                   | *yes*             | 11.49 ns      | 14.01 ns                  |
| `thread_local`          | no                |  1.38 ns      |  1.38 ns                  |
| `thread_local`          | *yes*             | 11.43 ns      | 14.02 ns                  |
| `dropping_thread_local` | *yes*             | 13.14 ns      | 31.14 ns                  |

Every lookup in the current implementation of `dropping_thread_local` requires calling `Arc::clone`.
This has significant overhead in its own right, so I benchmarked the other libraries both storing their data an regular `Box` vs. storing data in an `Arc` and doing `Arc::clone`.

On my Mac, the library ranges between 30% slower than calling `thread_local::ThreadLocal::get` + `Arc::clone` and 30x slower than a plain `std::thread_local!`. On my older Linux laptop, this library ranges between 3x slower than `thread_local::ThreadLocal::get` + `Arc::clone` and 60x slower than a plain `std::thread_local`.

This performance is a lot better than I expected (at least on the macbook). I am also disappointed by the performance of `Arc::clone`. Further improvements beyond this will almost certainly require amount of `unsafe` code. I have three ideas for improvement:

- Avoid requiring `Arc::clone` by using a [`LocalKey::with`] style API, and making `drop(DroppingThreadLocal)` delay freeing values from live threads until after that live thread dies.
- Use [biased reference counting] instead of an `Arc`. This would not require `unsafe` code directly in this crate. Unfortunately, biased reference counting can delay destruction or even leak if the heartbeat function is not called. Should also consider the [`trc` crate], which uses a similar idea.
- Using [`boxcar::Vec`] instead of a `HashMap` for lookup. This is essentially the same data structure that the `thread_local` crate uses, so should make the lookup performance similar.

[biased reference counting]: https://dl.acm.org/doi/10.1145/3243176.3243195
[`LocalKey::with`]: https://doc.rust-lang.org/std/thread/struct.LocalKey.html#method.with
[`boxcar::Vec`]: https://docs.rs/boxcar/0.2.13/boxcar/struct.Vec.html
[`trc` crate]: https://github.com/ericlbuehler/trc

### Locking
The implementation needs to acquire a global lock to initialize/deinitialize threads and create new locals.
Accessing thread-local data is also protected by a per-thread lock.
This lock should be uncontended, and [`parking_lot::Mutex`] should make this relatively fast.
I have been careful to make sure that locks are not held while user code is being executed.
This includes releasing locks before any destructors are executed.

## Limitations
The type that is stored must be `Send + Sync + 'static`.
The `Send` bound is necessary because the [`DroppingThreadLocal`] may be dropped from any thread.
The `Sync` bound is necessary to support snapshots,
and the `'static` bound is due to internal implementation chooses (use of safe code).

A Mutex can be used to work around the `Sync` limitation.
(I recommend [`parking_lot::Mutex`], which is optimized for uncontented locks)
You can attempt to use the [`fragile`] crate to work around the `Send` limitation,
but this will cause panics if the value is dropped from another thead.
Some ways a value can be dropped from another thread if a snapshot keeps the value alive,
or if the [`DroppingThreadLocal`] itself is dropped.

[`thread_local`]: https://docs.rs/thread_local/1.1/thread_local/
[`fragile`]: https://docs.rs/fragile/2/fragile/

<!-- cargo-rdme end -->

<!-- cargo inline doc references -->
[`std::thread_local!`]: https://doc.rust-lang.org/std/macro.thread_local.html
[`parking_lot::Mutex`]: https://docs.rs/parking_lot/latest/parking_lot/type.Mutex.html
[`DroppingThreadLocal`]: https://docs.rs/dropping-thread-local/latest/dropping-thread-local/struct.DroppingThreadLocal.html
[`DroppingThreadLocal::snapshot_iter`]: https://docs.rs/dropping-thread-local/latest/dropping-thread-local/struct.DroppingThreadLocal.html#method.snapshot_iter

## Prior Art
- [`per-thread-object`](https://github.com/quininer/per-thread-object) - I have not investigated this, but it appears very similar. By the maintainer of `io_uring`.
- [`thread_local`](https://github.com/Amanieu/thread_local-rs) - Discussed in documentation.
- [`std::thread::LocalKey`](https://doc.rust-lang.org/std/thread/struct.LocalKey.html) - Part of the stdlib. It is the abstraction upon which this crate is based.
   - The unstable [`#[thread_local]` attribute](https://github.com/rust-lang/rust/issues/29594) may be faster than a `LocalKey`

## License
Licensed under either the [Apache 2.0 License](./LICENSE-APACHE.txt) or [MIT License](./LICENSE-MIT.txt) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
