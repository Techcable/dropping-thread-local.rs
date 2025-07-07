//! Tests copied from the [`thread_local`] crate.
//!
//! The following behavioral changes have been made:
//! - Removed use of unsupported `clear` method (`same_thread` test)
//! - Iteration should not expect to see values from dead value (`iter` test)
//! - Remove `RefCell<String>` from test_sync - Currently `!Sync` types are unsupported
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::thread;

use dropping_thread_local::DroppingThreadLocal;

fn make_create() -> Arc<dyn Fn() -> usize + Send + Sync> {
    let count = AtomicUsize::new(0);
    Arc::new(move || count.fetch_add(1, Relaxed))
}

#[test]
fn same_thread() {
    let create = make_create();
    let tls = DroppingThreadLocal::new();
    assert_eq!(None, tls.get());
    assert_eq!("DroppingThreadLocal { local_data: None }", format!("{:?}", &tls));
    assert_eq!(0, *tls.get_or_init(|| create()));
    assert_eq!(Some(&0), tls.get().as_deref());
    assert_eq!(0, *tls.get_or_init(|| create()));
    assert_eq!(Some(&0), tls.get().as_deref());
    assert_eq!(0, *tls.get_or_init(|| create()));
    assert_eq!(Some(&0), tls.get().as_deref());
    assert_eq!("DroppingThreadLocal { local_data: Some(0) }", format!("{:?}", &tls));
    // DroppingThreadLocal::clear is currently not implemented
    #[cfg(any())]
    {
        tls.clear();
        assert_eq!(None, tls.get());
    }
}

#[test]
fn different_thread() {
    let create = make_create();
    let tls = Arc::new(DroppingThreadLocal::new());
    assert_eq!(None, tls.get());
    assert_eq!(0, *tls.get_or_init(|| create()));
    assert_eq!(Some(&0), tls.get().as_deref());

    let tls2 = tls.clone();
    let create2 = create.clone();
    thread::spawn(move || {
        assert_eq!(None, tls2.get());
        assert_eq!(1, *tls2.get_or_init(|| create2()));
        assert_eq!(Some(&1), tls2.get().as_deref());
    })
    .join()
    .unwrap();

    assert_eq!(Some(&0), tls.get().as_deref());
    assert_eq!(0, *tls.get_or_init(|| create()));
}

#[test]
fn iter() {
    let tls = Arc::new(DroppingThreadLocal::new());
    tls.get_or_init(|| Box::new(1));

    let tls2 = tls.clone();
    thread::spawn(move || {
        tls2.get_or_init(|| Box::new(2));
        let tls3 = tls2.clone();
        thread::spawn(move || {
            tls3.get_or_init(|| Box::new(3));
        })
        .join()
        .unwrap();
        drop(tls2);
    })
    .join()
    .unwrap();

    let tls = Arc::try_unwrap(tls).unwrap();

    let mut v = tls.snapshot_iter().map(|x| **x).collect::<Vec<i32>>();
    v.sort_unstable();
    // unlike the thread_local crate, dead values should be dropped b now
    assert_eq!(v, vec![1])
}

#[test]
fn test_drop_local() {
    let local = DroppingThreadLocal::new();
    struct Dropped(Arc<AtomicUsize>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.fetch_add(1, Relaxed);
        }
    }

    let dropped = Arc::new(AtomicUsize::new(0));
    local.get_or_init(|| Dropped(dropped.clone()));
    assert_eq!(dropped.load(Relaxed), 0);
    drop(local);
    assert_eq!(dropped.load(Relaxed), 1);
}

#[test]
fn is_sync() {
    fn foo<T: Sync>() {}
    foo::<DroppingThreadLocal<String>>();
    // currently, DroppingThreadLocal requires T: Sync
    #[cfg(any())]
    {
        foo::<DroppingThreadLocal<RefCell<String>>>();
    }
}
