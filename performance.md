# Performance Notes
Performance is much worse than the [`thread_local`] crate and [`std::thread_local`], but not terrible.

Every lookup in the current implementation of `dropping_thread_local` uses a hashmap and requires calling `Arc::clone`.

This performance is better than I expected (at least on my macbook). The biggest cost seems to be `Arc::clone`. Further improvements beyond this will almost certainly require amount of `unsafe` code. I have three ideas for improvement:

- Avoid requiring `Arc::clone` by using a [`LocalKey::with`] style API, and making `drop(DroppingThreadLocal)` delay freeing values from live threads until after that live thread dies.
- Use [biased reference counting] instead of an `Arc`. This would not require `unsafe` code directly in this crate. Unfortunately, biased reference counting can delay destruction or even leak if the heartbeat function is not called. The [`trc` crate] will not work, as `trc::Trc` is `!Send`.
- ~~Using [`boxcar::Vec`] instead of a `HashMap` for lookup.~~ This is essentially the same data structure that the `thread_local` crate uses, but is not an option because we need to modify existing entries.  UThe performance would not be that much better than a `parking_lot::Mutex<Vec<...>`, since it still requires an atomic load in the `get` function (as opposed to CAS + release store for a lock).

*NOTE*: Simply removing `Arc::clone` doesn't help that much. On the M1 mac it reduces the time to 9 ns. (I did this unsoundly, so it cant be published)

[biased reference counting]: https://dl.acm.org/doi/10.1145/3243176.3243195
[`LocalKey::with`]: https://doc.rust-lang.org/std/thread/struct.LocalKey.html#method.with
[`boxcar::Vec`]: https://docs.rs/boxcar/0.2.13/boxcar/struct.Vec.html
[`trc` crate]: https://github.com/ericlbuehler/trc

## Benchmark Results
A very basic benchmark on my M1 Mac and Linux Laptop (`i5-7200U` circa 2017) gives the following results:
| library                 | does `Arc::clone` | time (M1 Mac) | time (i5-7200U Laptop)    |
|-------------------------|-------------------|---------------|---------------------------|
| `std`                   | no                |  0.42 ns      |  0.69 ns                  |
| `std`                   | *yes*             | 11.49 ns      | 14.01 ns                  |
| `thread_local`          | no                |  1.38 ns      |  1.38 ns                  |
| `thread_local`          | *yes*             | 11.43 ns      | 14.02 ns                  |
| `dropping_thread_local` | *yes*             | 13.14 ns      | 31.14 ns                  |
| `Arc::clone`            | N/A               | 11.49 ns      | 11.67 ns                  |

On the M1 Mac, the library ranges between 30% slower than `thread_local::ThreadLocal::get` + `Arc::clone` and 30x slower than a plain `std::thread_local!` (on the Intel). On my older Linux laptop, this library ranges between 3x slower than `thread_local::ThreadLocal::get` + `Arc::clone` and 60x slower than a plain `std::thread_local`.

Surprisingly these results imply that [`Arc::clone`] is much more expensive the hashmap lookup, requiring 11ns by itself.

