use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};

use parking_lot::RwLock as ParkingRwLock;
use crossbeam::atomic::AtomicCell;
use arc_swap::ArcSwap;

use supersonic::safe::atomic_array::{AtomicArray, CopyType};

const OPS: usize = 1_000;

/// Spawn `threads` threads, each executing `f(tid)`
fn run_threads<F>(threads: usize, f: F)
where
    F: Fn(usize) + Send + Sync + 'static,
{
    let f = Arc::new(f);
    let mut handles = Vec::with_capacity(threads);

    for tid in 0..threads {
        let f = Arc::clone(&f);
        handles.push(thread::spawn(move || f(tid)));
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn atomic_array_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("AtomicArray");

    for &threads in &[1, 2, 4, 8, 16] {
        // ------------------------------------------------------------
        // Copy load (single slot)
        // ------------------------------------------------------------
        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, 42u64);

        group.bench_with_input(
            BenchmarkId::new("copy/read", threads),
            &threads,
            |b, &t| {
                let arr = Arc::clone(&arr);
                b.iter(|| {
                    let arr = Arc::clone(&arr);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            black_box(arr.load::<CopyType>(0));
                        }
                    });
                });
            },
        );

        // ------------------------------------------------------------
        // Copy update (single slot, max contention)
        // ------------------------------------------------------------
        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, 0u64);

        group.bench_with_input(
            BenchmarkId::new("copy/update", threads),
            &threads,
            |b, &t| {
                let arr = Arc::clone(&arr);
                b.iter(|| {
                    let arr = Arc::clone(&arr);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            arr.update::<_, CopyType>(0, |v: u64| v + 1);
                        }
                    });
                });
            },
        );

        // ------------------------------------------------------------
        // Slot-independent updates
        // ------------------------------------------------------------
        let slots = threads.max(1);
        let arr = Arc::new(AtomicArray::new(slots));
        for i in 0..slots {
            arr.store(i, 0u64);
        }

        group.bench_with_input(
            BenchmarkId::new("copy/update_independent", threads),
            &threads,
            |b, &t| {
                let arr = Arc::clone(&arr);
                b.iter(|| {
                    let arr = Arc::clone(&arr);
                    run_threads(t, move |tid| {
                        let idx = tid % slots;
                        for _ in 0..OPS {
                            arr.update::<_, CopyType>(idx, |v: u64| v + 1);
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

fn baseline_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Baselines");

    for &threads in &[1, 2, 4, 8, 16] {
        // ------------------------------------------------------------
        // Mutex
        // ------------------------------------------------------------
        let m = Arc::new(Mutex::new(0u64));

        group.bench_with_input(
            BenchmarkId::new("Mutex/write", threads),
            &threads,
            |b, &t| {
                let m = Arc::clone(&m);
                b.iter(|| {
                    let m = Arc::clone(&m);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            *m.lock().unwrap() += 1;
                        }
                    });
                });
            },
        );

        // ------------------------------------------------------------
        // Std RwLock
        // ------------------------------------------------------------
        let r = Arc::new(RwLock::new(0u64));

        group.bench_with_input(
            BenchmarkId::new("StdRwLock/write", threads),
            &threads,
            |b, &t| {
                let r = Arc::clone(&r);
                b.iter(|| {
                    let r = Arc::clone(&r);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            *r.write().unwrap() += 1;
                        }
                    });
                });
            },
        );

        // ------------------------------------------------------------
        // parking_lot RwLock
        // ------------------------------------------------------------
        let p = Arc::new(ParkingRwLock::new(0u64));

        group.bench_with_input(
            BenchmarkId::new("ParkingRwLock/write", threads),
            &threads,
            |b, &t| {
                let p = Arc::clone(&p);
                b.iter(|| {
                    let p = Arc::clone(&p);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            *p.write() += 1;
                        }
                    });
                });
            },
        );

        // ------------------------------------------------------------
        // AtomicCell
        // ------------------------------------------------------------
        let a = Arc::new(AtomicCell::new(0u64));

        group.bench_with_input(
            BenchmarkId::new("AtomicCell/fetch_add", threads),
            &threads,
            |b, &t| {
                let a = Arc::clone(&a);
                b.iter(|| {
                    let a = Arc::clone(&a);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            a.fetch_add(1);
                        }
                    });
                });
            },
        );

        // ------------------------------------------------------------
        // ArcSwap (read-heavy representative)
        // ------------------------------------------------------------
        let arc = Arc::new(ArcSwap::from_pointee(0u64));

        group.bench_with_input(
            BenchmarkId::new("ArcSwap/load", threads),
            &threads,
            |b, &t| {
                let arc = Arc::clone(&arc);
                b.iter(|| {
                    let arc = Arc::clone(&arc);
                    run_threads(t, move |_| {
                        for _ in 0..OPS {
                            let _ = arc.load();
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, atomic_array_bench, baseline_bench);
criterion_main!(benches);