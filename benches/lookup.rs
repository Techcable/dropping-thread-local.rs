#![allow(missing_docs)]
use std::hint::black_box;
use std::sync::Arc;

use dropping_thread_local::DroppingThreadLocal;
use thread_local::ThreadLocal;

fn main() {
    let mut c = criterion::Criterion::default().configure_from_args();

    c.bench_function("std::get", |b| {
        std::thread_local!(static LOCAL: Box<i32> = Box::new(0));
        LOCAL.with(|x| black_box(**x));
        b.iter(|| {
            black_box(LOCAL.with(|x| **x));
        });
    });

    c.bench_function("std::get_arc", |b| {
        std::thread_local!(static LOCAL: Arc<i32> = Arc::new(0));
        LOCAL.with(|x| black_box(**x));
        b.iter_with_large_drop(|| {
            black_box(LOCAL.with(|x| Arc::clone(x)));
        });
    });

    c.bench_function("thread_local::get", |b| {
        let local = ThreadLocal::new();
        local.get_or(|| Box::new(0));
        b.iter(|| {
            black_box(local.get());
        });
    });

    c.bench_function("thread_local::get_arc", |b| {
        let local = ThreadLocal::new();
        local.get_or(|| Arc::new(0));
        b.iter_with_large_drop(|| {
            black_box(Arc::clone(local.get().unwrap()));
        });
    });

    c.bench_function("dropping_thread_local::get", |b| {
        let local = DroppingThreadLocal::new();
        local.get_or_init(|| Box::new(0));
        b.iter_with_large_drop(|| {
            black_box(local.get());
        });
    });

    c.bench_function("Arc::clone", |b| {
        let arc = Arc::new(7);
        b.iter(|| Arc::clone(&arc));
    });
}
