# Changelog

Notable changes to this project should be documented in this file.
Make sure it is up to date before performing a release.

This project follows the [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) format wherever that is reasonable.

The "title" of each release should be its first line.
A title is required for publishing a github release, so all versions should have one.

Most changes include the relevant [jj](https://jj-vcs.dev) change ids in parens. An example of a change id is wuoxvnsw.

## Unreleased

### Changed
- Update `imbl` from v6 to v7, fixing vulnerability in `imbl` dependency (skrttvow)
  - Avoids [`RUSTSEC-2026-0292`], a double-free vulnerability in `imbl-sized-chunks`
  - Note that there is no vulnerability on `dropping-thread-local` itself.
    This crate does not use unsafe code and so cannot have memory safety problems.
  - Avoids [`RUSTSEC-2026-0247`] warning that `bitmaps` is unmaintained (not a security problem)
- Allow testing with criterion v0.7 in addition to v0.8 (uzknwvlo)
  - Needed to build tests on MSRV of 1.85, because v0.8 requires Rust 1.86
- Support both versions of parking_lot v0.12 and v0.13 (kpwkmwsn)
  - Cargo will select the best version based upon your MSRV
  - Does not affect the public API in any way, so may be changed or removed in the future.
  - It is easy to support multiple versions since the only method we use is `Mutex::lock`.

[`RUSTSEC-2026-0292`]: https://rustsec.org/advisories/RUSTSEC-2026-0292.html
[`RUSTSEC-2026-0247`]: https://rustsec.org/advisories/RUSTSEC-2026-0247.html

### Added
- Setup `cargo-deny` to check for security and licensing issues (qrqzwvyv)
- Setup Github Actions for CI (unruoooy)

### Fixed
- Fix typos and grammar in documentation and comments (nwlpmopk)

### Documented
- Document that accessing a thread local while a thread is being destroyed may panic (uxvxsozt)

## v0.1.5 - 2026-01-13
Upgrade to foldhash v0.2.

### Changes
- Upgrade `foldhash` from v0.1 to v0.2 (6364b313)

## v0.1.4 - 2025-08-14
Update intid to v0.3.0-beta.1, fixing breakage.

### Changes
- Update intid to v0.3.0-beta.1, fixing breakage (769394f6)
  - The intid crate is now in beta, so I don't plan to make any more breaking changes.

## v0.1.3 - 2025-08-14
Replace internal `HashMap` with `Vec`.

Performance improvements are less than stellar (within 10%), but still worthwhile.

### Changed
- perf: Replace `HashMap` with a `Vec` (731d4915)
  - Appropriately update the performance comment in `LiveThreadState` (b9aa0dbc)
- Drop unused `idmap` dependency (0a9b6ddb)
  - Commit message is wrong and says `intid` instead of `idmap`.
- Move performance notes and benchmarks to a separate file, avoiding clutter in the main docs/readme. (19f10e6d)

### Fixed
- Fix index error in `DroppingThreadLocal::drop` (a4768aa5)

### Documented
- perf: Comment on inability to use `boxcar::Vec` (3707a64e)

## v0.1.2 - 2025-08-13
Update `imbl` from v5.0 to v6.0

Nothing else changed.

### Changed
- Update `imbl` from v5.0 to v6.0 (353c991b)

## v0.1.1 - 2025-08-10
Add set method to `DroppingThreadLocal`.

### Added
- Add [`DroppingThreadLocal::set`] method (d74981d0)

### Documented
- Comment on performance of unsafely removing `Arc::clone` (0d75ac56)

[`DroppingThreadLocal::set`]: https://docs.rs/dropping-thread-local/0.1/dropping_thread_local/struct.DroppingThreadLocal.html#method.set

## v0.1.0 - 2025-08-09
Add get_or_try_init method

Crate no longer marked as alpha.

### Added
- Add get_or_try_init method (a1726907)
- Add basic lookup benchmark (d1f000a0)

## v0.1.0-alpha.0 - 2025-06-06
Initial release.

Implemented entirely in safe code. Passes basic tests.
