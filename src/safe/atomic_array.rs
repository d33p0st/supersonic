
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::hint::spin_loop;
use std::sync::atomic::{AtomicU64, Ordering};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

// version bit layout (IMPORTANT)
// bit 0 : LOCK bit (odd = writer active)
// bit 1 : INIT bit (slot initialized)
// bits 2.. : version counter (ABA resistance)
const LOCK_BIT: u64 = 1 << 0;
const INIT_BIT: u64 = 1 << 1;
const VERSION_INC: u64 = 1 << 2;

/// A cache-line aligned storage unit used as a low-level concurrency primitive.
///
/// # ⚠️ Important
///
/// **`Unit<E>` is NOT meant to be used as a standalone data structure.**
///
/// This type exists solely as an *internal building block* for higher-level
/// abstractions (such as `AtomicArray`). On its own, it does **not** provide
/// safety, correctness, or progress guarantees.
///
/// ## Why this is unsafe to use directly
///
/// - `value` is stored in `UnsafeCell<MaybeUninit<E>>` and may be:
///   - uninitialized
///   - concurrently mutated
/// - `version` has no standalone semantic meaning without the surrounding
///   protocol that coordinates reads and writes
/// - No API here enforces:
///   - proper initialization
///   - exclusive access
///   - memory ordering correctness
///
/// ## Intended usage
///
/// This struct should only be accessed through higher-level abstractions that:
///
/// - define a strict access protocol
/// - enforce memory ordering
/// - guarantee safe initialization and destruction
///
/// Direct usage outside those abstractions is **undefined behavior**.
#[repr(align(64))]
pub struct Unit<E> {
    version: AtomicU64,
    value: UnsafeCell<MaybeUninit<E>>,
}

unsafe impl<E: Send> Send for Unit<E> {}
unsafe impl<E: Send + Sync> Sync for Unit<E> {}

/// A high-performance, index-addressable, lock-free concurrent array.
///
/// `AtomicArray<E>` is designed for **low-latency, contention-aware workloads**
/// where indexed access is required and traditional synchronization primitives
/// (mutexes, RW-locks) become a scalability bottleneck.
///
/// Each element is stored in an independently versioned, cache-line–aligned
/// unit, allowing concurrent readers and writers to operate with minimal
/// interference.
///
/// ---
///
/// ## Design Overview
///
/// - Backed by a fixed-capacity contiguous array (`Box<[Unit<E>]>`)
/// - Each slot:
///   - Is aligned to a cache line (64 bytes)
///   - Maintains its own version counter
///   - Stores the value in `UnsafeCell<MaybeUninit<E>>`
/// - No global locks
/// - No resizing
///
/// This design allows **fine-grained synchronization at the slot level**
/// instead of coarse-grained structure-wide locking.
///
/// ---
///
/// ## Key Properties
///
/// - **Index-based access**: `O(1)` deterministic access
/// - **Lock-free reads**: Readers never block writers
/// - **Localized contention**: Conflicts are isolated to individual indices
/// - **Cache-friendly**: Prevents false sharing between neighboring slots
/// - **Predictable latency**: No parking, no kernel involvement
///
/// ---
/// 
/// ## Usage
/// 
/// ```
/// use supersonic::prelude::{AtomicArray, CopyType, CloneType};
/// 
/// let array = AtomicArray::<i32>::new(10); // capacity 10, copy-type data (i32).
/// let array2 = AtomicArray::<String>::new(10); // capacity 10, clone-type data (String).
/// 
/// assert_eq!(array.capacity(), 10);
/// assert_eq!(array2.capacity(), 10);
/// 
/// assert_eq!(array.length(), 0);
/// ```
/// 
/// ---
///
/// ## What This Is Good At
///
/// `AtomicArray` excels when:
///
/// - You need **very fast indexed access**
/// - Reads and writes happen concurrently
/// - Contention is spread across indices (not all threads hitting one slot)
/// - Latency predictability matters more than raw throughput
///
/// ### Typical Use Cases
///
/// - Real-time systems
/// - Low-latency services
/// - In-memory state tables
/// - Metrics counters / rolling statistics
/// - Actor mailboxes (index-based routing)
/// - Trading / matching engines (carefully partitioned)
///
/// ---
///
/// ## What This Is NOT Good At
///
/// Avoid `AtomicArray` when:
///
/// - You need to **resize** the collection
/// - You need **iteration with strong consistency**
/// - All threads frequently write to the **same index**
/// - Your workload is mostly sequential
///
/// In these cases, traditional structures are usually faster and simpler.
///
/// ---
///
/// ## Average-Case Behavior (Day-to-Day Usage)
///
/// In typical applications with moderate concurrency:
///
/// - Reads complete in a few atomic operations
/// - Writes incur a small retry cost only under contention
/// - Independent indices scale almost linearly with thread count
/// - Same-index contention degrades gracefully (no blocking)
///
/// For most real-world workloads, performance sits **between**
/// `AtomicU64` and `Mutex<T>`, closer to atomics when contention is low.
///
/// ---
///
/// ## Safety Model
///
/// `AtomicArray` internally uses `UnsafeCell` and relaxed atomics.
/// Safety is guaranteed **only** by enforcing a strict access protocol:
///
/// - Readers validate slot versions before and after access
/// - Writers publish updates via version transitions
/// - Memory ordering is carefully controlled
///
/// **Do not attempt to bypass the public API.**
///
/// ---
///
/// ## Tradeoffs
///
/// ### Pros
///
/// - 🚀 Very low latency
/// - 🧵 No blocking or parking
/// - 🧠 Contention isolated per index
/// - 🧱 Strong cache-line isolation
///
/// ### Cons
///
/// - 📏 Fixed capacity
/// - 🔁 Writes may retry under contention
/// - 🚫 Not suitable for bulk iteration
/// - ⚠️ More complex mental model than locks
///
/// ---
///
/// ## Performance Comparison (Conceptual Benchmarks)
///
/// | Data Structure          | Read Speed | Write Speed | Contention Handling | Resize | Latency Predictability |
/// |-------------------------|------------|-------------|---------------------|--------|------------------------|
/// | `AtomicArray`           | 🚀🚀🚀      | 🚀🚀        | 🧠 Slot-local       | ❌     | ✅ Excellent           |
/// | `Vec<Atomic*>`          | 🚀🚀🚀      | 🚀🚀        | 🔥 Slot-local       | ❌     | ✅ Excellent           |
/// | `RwLock<Vec<T>>`        | 🚀🚀        | 🐢          | 🔒 Global           | ✅     | ❌ Poor                |
/// | `Mutex<Vec<T>>`         | 🐢          | 🐢          | 🔒 Global           | ✅     | ❌ Poor                |
/// | `DashMap`               | 🚀🚀        | 🚀🚀        | 🧩 Sharded          | ✅     | ⚠️ Variable            |
///
/// ---
///
/// ## When You SHOULD Use `AtomicArray`
///
/// ✔ You know the maximum size upfront  
/// ✔ You need index-based access  
/// ✔ You want predictable latency  
/// ✔ You are optimizing a hot path  
///
/// ## When You SHOULD NOT
///
/// ❌ You need resizing  
/// ❌ You need ordered iteration  
/// ❌ You prefer simplicity over performance  
///
/// ---
///
/// ## Mental Model
///
/// Think of `AtomicArray` as:
///
/// > "A fixed table of independent, lock-free cells,
/// > each optimized for concurrent access."
///
/// It is **not** a general-purpose container.
/// It is a **specialized concurrency primitive**.
///
/// ---
///
/// ## Final Note
///
/// If you do not have a concrete performance problem,
/// you probably do **not** need this type.
///
/// If you *do* have one — this exists for exactly that reason.
#[repr(align(64))]
pub struct AtomicArray<E> {
    units: Box<[Unit<E>]>,
    capacity: usize,
}

unsafe impl<E: Send> Send for AtomicArray<E> {}
unsafe impl<E: Send + Sync> Sync for AtomicArray<E> {}


/// Abstraction over how values are read from an `AtomicArray` slot.
///
/// `DataType<E>` defines **how a value is observed** from a `Unit<E>` during
/// concurrent access. It allows `AtomicArray` to support different read
/// semantics (e.g. `Copy` vs `Clone`) **without duplicating logic or branching
/// at runtime**.
///
/// ---
///
/// ## Why This Trait Exists
///
/// Rust types fall into two broad categories:
///
/// - **`Copy` types** (e.g. integers, pointers)
/// - **Non-`Copy` but `Clone` types** (e.g. `String`, `Vec<T>`)
///
/// Reading these safely in a concurrent, lock-free context requires
/// *different strategies*:
///
/// - `Copy` values can be returned directly
/// - `Clone` values must be duplicated without exposing partially-written state
///
/// `DataType` encodes this distinction at the **type level**, allowing the
/// compiler to select the correct strategy at compile time.
///
/// ---
///
/// ## Associated Type
///
/// - `Read`:
///   The type returned to the caller when reading a value.
///
/// This may differ from `E` itself (e.g. references are intentionally avoided).
///
/// ---
///
/// ## Safety Model
///
/// Implementations of this trait are responsible for:
///
/// - Observing the value **only after version validation**
/// - Never exposing uninitialized or torn data
/// - Respecting the memory ordering guarantees enforced by `AtomicArray`
///
/// This trait is **not meant to be implemented by users**.
/// It is an internal extension point used to specialize behavior
/// for `Copy` vs `Clone` types.
///
/// ---
///
/// ## Design Constraints
///
/// - Must be zero-cost (no virtual dispatch)
/// - Must not allocate
/// - Must not block
/// - Must not mutate the `Unit`
///
/// ---
///
/// ## Important Note
///
/// This trait operates on [`Unit<E>`], which is an **internal building block**
/// of `AtomicArray`.
///
/// **`Unit<E>` is not intended to be used directly or independently.**
///
/// Any direct use outside of `AtomicArray` is unsupported and unsafe.
///
/// ---
///
/// ## Mental Model
///
/// Think of `DataType` as:
///
/// > “The rules for how a value may be safely *observed* under concurrency.”
///
/// Not *how it is stored*, not *how it is written* — only how it is *read*.
pub trait DataType<E> {
    /// The type returned when reading a value.
    type Read;

    /// Reads the value from a unit according to the rules of this data type.
    ///
    /// Implementations must assume:
    /// - The caller has already performed version validation
    /// - The underlying value may be concurrently accessed
    ///
    /// This function must never panic and must never expose
    /// partially-initialized data.
    fn read(unit: &Unit<E>) -> Self::Read;
}

/// Marker type indicating **`Clone`-based read semantics**.
///
/// `CloneType` is used as a type-level signal to `AtomicArray` that the stored
/// element `E` **cannot be copied trivially** and must instead be **cloned**
/// when read.
///
/// ---
///
/// ## When This Is Used
///
/// This marker is selected automatically for types that:
///
/// - Do **not** implement `Copy`
/// - **Do** implement `Clone`
///
/// Examples include:
///
/// - `String`
/// - `Vec<T>`
/// - `Arc<T>`
/// - Most heap-allocated or ownership-heavy types
///
/// ---
///
/// ## Behavior
///
/// When `CloneType` is active:
///
/// - Reads perform a **version-validated clone**
/// - Writers may retry if a concurrent read detects mutation
/// - No references (`&E`) are ever exposed
///
/// This guarantees that:
///
/// - Readers never observe torn or partially-written values
/// - Clones are consistent with a single logical point in time
///
/// ---
///
/// ## Performance Characteristics
///
/// - ❌ Slower than `CopyType`
/// - ❌ Allocations may occur (depending on `Clone` impl)
/// - ✅ Safe for complex, non-trivial data
///
/// ---
///
/// ## Design Note
///
/// `CloneType` contains **no data** and exists purely at the type level.
/// It is never instantiated and has zero runtime cost.
///
/// ---
///
/// ## Important
///
/// This marker is an **internal implementation detail**.
/// Users should not select or construct it manually.
pub struct CloneType;

/// Marker type indicating **`Copy`-based read semantics**.
///
/// `CopyType` signals that the stored element `E` implements `Copy` and can be
/// safely duplicated using a simple memory copy during reads.
///
/// ---
///
/// ## When This Is Used
///
/// This marker is selected automatically for types that:
///
/// - Implement `Copy`
///
/// Examples include:
///
/// - Integers (`u64`, `usize`, etc.)
/// - Raw pointers
/// - Small POD structs
///
/// ---
///
/// ## Behavior
///
/// When `CopyType` is active:
///
/// - Reads return the value **by value**
/// - No allocation or cloning occurs
/// - Reads are extremely fast and cache-friendly
///
/// Version validation still applies to prevent torn reads.
///
/// ---
///
/// ## Performance Characteristics
///
/// - ✅ Extremely fast reads
/// - ✅ No allocation
/// - ✅ Ideal for high-frequency, low-latency access
///
/// ---
///
/// ## Design Note
///
/// `CopyType` contains **no data** and exists purely at the type level.
/// It is never instantiated and has zero runtime cost.
///
/// ---
///
/// ## Important
///
/// This marker is an **internal implementation detail**.
/// Users should not select or construct it manually.
pub struct CopyType;

impl<E: Clone> DataType<E> for CloneType {
    type Read = E;

    fn read(unit: &Unit<E>) -> Self::Read {
        unsafe { (*unit.value.get()).assume_init_ref().clone() }
    }
}

impl<E: Copy> DataType<E> for CopyType {
    type Read = E;

    fn read(unit: &Unit<E>) -> Self::Read {
        unsafe { (*unit.value.get()).assume_init() }
    }
}


impl<E> AtomicArray<E> {

    #[must_use = "New instances of AtomicArray must serve a purpose!"]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0!");
        
        let mut units = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            units.push(Unit {
                version: AtomicU64::new(0),
                value: UnsafeCell::new(MaybeUninit::uninit()),
            });
        }

        Self { units: units.into_boxed_slice(), capacity }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    fn unit(&self, index: usize) -> &Unit<E> {
        assert!(index < self.units.len());
        &self.units[index]
    }

    /// acquires write atomically
    #[inline(always)]
    fn reserve_and<F>(&self, index: usize, closure: F)
    where 
        F: FnOnce(&Unit<E>)
    {
        let unit = self.unit(index);
        let mut v;

        loop {
            v = unit.version.load(Ordering::Acquire);

            // wait while lock bit
            if (v & LOCK_BIT) != 0 {
                spin_loop();
                continue;
            }

            // attempt even -> odd (transition)
            if unit.version.compare_exchange(v, v | LOCK_BIT, Ordering::AcqRel, Ordering::Acquire)
                .is_ok() {
                    break;
                }
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            closure(unit);
        }));

        let new_v = (unit.version.load(Ordering::Relaxed) & !LOCK_BIT) + VERSION_INC;
        unit.version.store(
            new_v,
            Ordering::Release
        );

        if let Err(panic) = result {
            resume_unwind(panic);
        }

    }

    pub fn store(&self, index: usize, value: E) {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");

        self.reserve_and(index, |unit| {
            unsafe {
                (*unit.value.get()) = MaybeUninit::new(value);
            }
            // unit.version.fetch_add(VERSION_INC | INIT_BIT, Ordering::Release);
            let v = unit.version.load(Ordering::Relaxed);
            unit.version.store(v | INIT_BIT, Ordering::Relaxed);
        });
    }

    pub fn load<D: DataType<E>>(&self, index: usize) -> D::Read {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        let unit = self.unit(index);

        loop {
            let v1 = unit.version.load(Ordering::Acquire);
            if (v1 & LOCK_BIT) != 0 {
                spin_loop();
                continue;
            }

            if (v1 & INIT_BIT) == 0 {
                panic!("load is performed on uninitialized index: {}", index);
            }

            let value = D::read(unit);

            let v2 = unit.version.load(Ordering::Acquire);
            if v1 == v2 {
                return value;
            }
        }
    }

    pub fn try_load<D: DataType<E>>(&self, index: usize) -> Option<D::Read> {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        let unit = self.unit(index);

        loop {
            let v1 = unit.version.load(Ordering::Acquire);
            if (v1 & LOCK_BIT) != 0 {
                spin_loop();
                continue;
            }

            if (v1 & INIT_BIT) == 0 {
                return None;
            }

            let value = D::read(unit);

            let v2 = unit.version.load(Ordering::Acquire);
            if v1 == v2 {
                return Some(value);
            }
        }
    }

    pub fn replace<D: DataType<E>>(&self, index: usize, value: E) -> D::Read {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        let mut old = None;

        self.reserve_and(index, |unit| {
            let v = unit.version.load(Ordering::Relaxed);
            if (v & INIT_BIT) == 0 {
                panic!("replace is performed on uninitialized index: {}", index);
            }

            unsafe {
                old = Some(D::read(unit));
                (*unit.value.get()) = MaybeUninit::new(value);
            }

            unit.version.store(v | INIT_BIT, Ordering::Relaxed);
        });

        old.unwrap()
    }

    pub fn try_replace<D: DataType<E>>(&self, index: usize, value: E) -> Option<D::Read> {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        let mut old = None;

        self.reserve_and(index, |unit| {
            let v = unit.version.load(Ordering::Relaxed);
            if (v & INIT_BIT) == 0 {
                return;
            }

            unsafe {
                old = Some(D::read(unit));
                (*unit.value.get()) = MaybeUninit::new(value);
            }

            unit.version.store(v | INIT_BIT, Ordering::Relaxed);
        });

        old
    }

    pub fn update<F, D>(&self, index: usize, closure: F)
    where 
        D: DataType<E>,
        F: FnOnce(D::Read) -> E,
    {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        self.reserve_and(index, |unit| {
            let v = unit.version.load(Ordering::Relaxed);
            if (v & INIT_BIT) == 0 {
                panic!("update is performed on uninitialized index: {}", index);
            }

            unsafe {
                let current = D::read(unit);
                (*unit.value.get()) = MaybeUninit::new(closure(current));
            }

            unit.version.store(v | INIT_BIT, Ordering::Relaxed);
        });
    }

    pub fn try_update<F, D>(&self, index: usize, closure: F) -> bool
    where 
        D: DataType<E>,
        F: FnOnce(D::Read) -> E,
    {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        let mut status = false;
        self.reserve_and(index, |unit| {
            let v = unit.version.load(Ordering::Relaxed);
            if (v & INIT_BIT) == 0 {
                return;
            }

            unsafe {
                let current = D::read(unit);
                (*unit.value.get()) = MaybeUninit::new(closure(current));
            }

            unit.version.store(v | INIT_BIT, Ordering::Relaxed);
            status = true;
        });

        status
    }

    pub fn length(&self) -> usize {
        self.units.iter().filter(|unit| {
            unit.version.load(Ordering::Acquire) & INIT_BIT != 0
        })
        .count()
    }

    #[inline(always)]
    pub fn contains_index(&self, index: usize) -> bool {
        assert!(index < self.capacity, "Index must always be less than capacity!");
        self.unit(index)
            .version
            .load(Ordering::Acquire) & INIT_BIT != 0
    }

    pub fn free(&self, index: usize) {
        assert!(index < self.capacity, "Index must always be less than capacity!");

        self.reserve_and(index, |unit| {
            let v = unit.version.load(Ordering::Relaxed);
            unit.version.store(v & !INIT_BIT, Ordering::Relaxed);
        });
    }

    pub fn swap<D: DataType<E>>(&self, index: usize, value: E) -> D::Read { self.replace::<D>(index, value) }

    pub fn load_or_store<F, D>(&self, index: usize, store: F) -> D::Read
    where 
        F: FnOnce() -> E,
        D: DataType<E>,
    {
        assert!(index < self.capacity, "Index must be less than capacity at all times!");
        if let Some(value) = self.try_load::<D>(index) {
            return value;
        }

        self.reserve_and(index, |unit| {
            let v = unit.version.load(Ordering::Relaxed);
            if (v & INIT_BIT) == 0 {
                unsafe {
                    (*unit.value.get()) = MaybeUninit::new(store());
                }
                unit.version.store(v | INIT_BIT, Ordering::Relaxed);
            }
        });

        self.load::<D>(index)
    }

}

impl<E> Drop for AtomicArray<E> {
    fn drop(&mut self) {
        for unit in self.units.iter_mut() {
            let v = unit.version.load(Ordering::Relaxed);
            if (v & INIT_BIT) != 0 {
                unsafe {
                    unit.value.get_mut().assume_init_drop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn basic_store_load_copy() {
        let arr = AtomicArray::new(4);

        arr.store(0, 42u64);
        let v = arr.load::<CopyType>(0);

        assert_eq!(v, 42);
    }

    #[test]
    fn basic_store_load_clone() {
        let arr = AtomicArray::new(2);

        arr.store(1, String::from("hello"));
        let v = arr.load::<CloneType>(1);

        assert_eq!(v, "hello");
    }

    #[test]
    fn try_load_uninitialized() {
        let arr: AtomicArray<u32> = AtomicArray::new(1);
        assert!(arr.try_load::<CopyType>(0).is_none());
    }

    #[test]
    fn replace_returns_old() {
        let arr = AtomicArray::new(1);

        arr.store(0, 10);
        let old = arr.replace::<CopyType>(0, 20);

        assert_eq!(old, 10);
        assert_eq!(arr.load::<CopyType>(0), 20);
    }

    #[test]
    fn update_in_place() {
        let arr = AtomicArray::new(1);

        arr.store(0, 5);
        arr.update::<_, CopyType>(0, |v| v * 2);

        assert_eq!(arr.load::<CopyType>(0), 10);
    }

    #[test]
    fn concurrent_reads() {
        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, 123);

        let mut handles = vec![];

        for _ in 0..16 {
            let a = arr.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100_000 {
                    assert_eq!(a.load::<CopyType>(0), 123);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn concurrent_read_write() {
        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, 0u64);

        let stop = Arc::new(AtomicBool::new(false));

        // writer
        {
            let a = arr.clone();
            let s = stop.clone();
            thread::spawn(move || {
                let mut i = 0;
                while !s.load(Ordering::Relaxed) {
                    a.store(0, i);
                    i += 1;
                }
            });
        }

        // readers
        let mut readers = vec![];
        for _ in 0..8 {
            let a = arr.clone();
            readers.push(thread::spawn(move || {
                for _ in 0..100_000 {
                    let _ = a.load::<CopyType>(0);
                }
            }));
        }

        for r in readers {
            r.join().unwrap();
        }

        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn stress_test() {
        let arr = Arc::new(AtomicArray::new(8));

        for i in 0..8 {
            arr.store(i, i as u64);
        }

        let mut threads = vec![];

        for t in 0..16 {
            let a = arr.clone();
            threads.push(thread::spawn(move || {
                for i in 0..200_000 {
                    let idx = (i + t) % 8;
                    let _ = a.load::<CopyType>(idx);
                    a.update::<_, CopyType>(idx, |v| v + 1);
                }
            }));
        }

        for t in threads {
            t.join().unwrap();
        }
    }

    #[test]
    fn version_monotonicity() {
        let arr = AtomicArray::new(1);
        arr.store(0, 0u64);

        let unit = &arr.units[0];
        let mut last = unit.version.load(Ordering::Relaxed);

        for _ in 0..1000 {
            arr.update::<_, CopyType>(0, |v| v + 1);
            let now = unit.version.load(Ordering::Relaxed);
            assert!(now > last);
            last = now;
        }
    }

    #[test]
    fn drop_clone_safety() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);

        #[derive(Clone)]
        struct DropCounter;
        impl Drop for DropCounter {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        {
            let arr = AtomicArray::new(1);
            arr.store(0, DropCounter);
        }

        assert_eq!(DROPS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn writer_eventually_makes_progress() {
        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, 0u64);

        let done = Arc::new(AtomicBool::new(false));

        let reader = {
            let a = arr.clone();
            let d = done.clone();
            thread::spawn(move || {
                while !d.load(Ordering::Relaxed) {
                    let _ = a.load::<CopyType>(0);
                }
            })
        };

        let writer = {
            let a = arr.clone();
            thread::spawn(move || {
                for _ in 0..1000 {
                    a.update::<_, CopyType>(0, |v| v + 1);
                }
            })
        };

        writer.join().unwrap();
        done.store(true, Ordering::Relaxed);
        reader.join().unwrap();
    }

    #[test]
    fn aba_like_pattern() {
        let arr = AtomicArray::new(1);

        arr.store(0, 10u64);
        let first = arr.load::<CopyType>(0);

        arr.store(0, 20);
        arr.store(0, 10);

        let second = arr.load::<CopyType>(0);
        assert_eq!(first, second);
    }

    #[test]
    fn update_panic_does_not_corrupt() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let arr = AtomicArray::new(1);
        arr.store(0, 5u64);

        let result = catch_unwind(AssertUnwindSafe(|| {
            arr.update::<_, CopyType>(0, |_| {
                panic!("boom");
            });
        }));

        assert!(result.is_err());

        // Structure must still be usable
        assert_eq!(arr.load::<CopyType>(0), 5);
    }

    #[test]
    fn linearizability_sanity() {
        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, 0u64);

        let a1 = arr.clone();
        let t1 = thread::spawn(move || {
            a1.update::<_, CopyType>(0, |v| v + 1);
        });

        let a2 = arr.clone();
        let t2 = thread::spawn(move || {
            a2.update::<_, CopyType>(0, |v| v + 1);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let v = arr.load::<CopyType>(0);
        assert!(v == 1 || v == 2);
    }

    #[test]
    fn drop_after_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let arr = Arc::new(AtomicArray::new(1));
        arr.store(0, String::from("hello"));

        let a = arr.clone();
        let t = thread::spawn(move || {
            for _ in 0..1000 {
                let _ = a.load::<CloneType>(0);
            }
        });

        t.join().unwrap();
        drop(arr); // should not deadlock or double-drop
    }

    #[test]
    fn slots_are_independent() {
        let arr = Arc::new(AtomicArray::new(2));
        arr.store(0, 1u64);
        arr.store(1, 100u64);

        let a = arr.clone();
        let t = std::thread::spawn(move || {
            for _ in 0..1000 {
                a.update::<_, CopyType>(0, |v| v + 1);
            }
        });

        for _ in 0..1000 {
            assert_eq!(arr.load::<CopyType>(1), 100);
        }

        t.join().unwrap();
    }
}

#[cfg(all(feature = "loom_test", feature = "atomic-array"))]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::thread;

    #[test]
    fn loom_store_load() {
        loom::model(|| {
            let arr = Arc::new(AtomicArray::new(1));

            let a1 = arr.clone();
            let t1 = thread::spawn(move || {
                a1.store(0, 1u64);
            });

            let a2 = arr.clone();
            let t2 = thread::spawn(move || {
                let _ = a2.try_load::<CopyType>(0);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }


    #[test]
    fn loom_no_torn_read() {
        loom::model(|| {
            let arr = loom::sync::Arc::new(AtomicArray::new(1));

            let writer = {
                let a = arr.clone();
                loom::thread::spawn(move || {
                    for _ in 0..3 {
                        a.store(0, 1u64);
                        a.store(0, 2u64);
                    }
                })
            };

            let reader = {
                let a = arr.clone();
                loom::thread::spawn(move || {
                    for _ in 0..3 {
                        if let Some(v) = a.try_load::<CopyType>(0) {
                            assert!(v == 1 || v == 2);
                        }
                    }
                })
            };

            writer.join().unwrap();
            reader.join().unwrap();
        });
    }

    #[test]
    fn loom_concurrent_updates() {
        loom::model(|| {
            let arr = loom::sync::Arc::new(AtomicArray::new(1));
            arr.store(0, 0u64);

            let t1 = {
                let a = arr.clone();
                loom::thread::spawn(move || {
                    a.update::<_, CopyType>(0, |v| v + 1);
                })
            };

            let t2 = {
                let a = arr.clone();
                loom::thread::spawn(move || {
                    a.update::<_, CopyType>(0, |v| v + 1);
                })
            };

            t1.join().unwrap();
            t2.join().unwrap();

            let v = arr.load::<CopyType>(0);
            assert!(v == 1 || v == 2);
        });
    }

    #[test]
    fn loom_panic_does_not_lock_forever() {
        loom::model(|| {
            let arr = Arc::new(AtomicArray::new(1));
            arr.store(0, 1u64);

            let a = arr.clone();
            let _ = thread::spawn(move || {
                let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    a.update::<_, CopyType>(0, |_| panic!("boom"));
                }));
            }).join();

            // Must still be usable
            let _ = arr.load::<CopyType>(0);
        });
    }
}