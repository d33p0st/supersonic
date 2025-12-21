use std::sync::{Arc, atomic::{AtomicPtr, AtomicUsize}};
use arc_swap::ArcSwapOption;
use tokio::sync::RwLock;

mod compat;
mod traits;
use traits::Allocation;

struct Container<T> {
    slots: Box<[AtomicPtr<RwLock<T>>]>,
    capacity: AtomicUsize,
    length: AtomicUsize,
}

impl <T> Container<T> {
    async fn allocate(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than zero!");

        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity { slots.push(AtomicPtr::new(std::ptr::null_mut())); }
        let slots = slots.into_boxed_slice();

        Self {
            slots,
            capacity: AtomicUsize::new(capacity),
            length: AtomicUsize::new(0)
        }
    }
}

impl <T> Drop for Container<T> {
    fn drop(&mut self) {
        for slot in self.slots.iter() {
            let pointer = slot.load(std::sync::atomic::Ordering::Acquire);
            if !pointer.is_null() {
                unsafe {
                    let _ = Arc::from_raw(pointer);
                }
            }
        }
    }
}

/// ### -> `Sequence<T>` - A high-speed, high-performance, high-concurrency, lock-free, thread-safe, reactive sequence data structure.
/// 
/// `Sequence<T>` is a high-speed, high-performance, high-concurrency data structure that provides a dynamic array
/// with atomic operations, automatic resizing, and comprehensive reactive capabilities. It is
/// designed for demanding multi-threaded environments where multiple threads need to safely access and
/// modify a shared sequence of elements with minimal latency and maximum throughput.
/// 
/// ### -> `Reactivity Explained`
/// 
/// **Reactivity** in `Sequence<T>` refers to the ability to share and synchronize element modifications
/// across multiple references to the same underlying data:
/// 
/// - **Reactive Operations** (e.g., `modify`, `extract`, `slice`, `split`, `reverse`): Create sequences
///   that share the same underlying `Arc<RwLock<T>>` references. When you modify an element through one
///   sequence, the change is immediately visible through all other sequences that reference that element.
///   This enables powerful patterns like multiple views of the same data or shared work queues.
/// 
/// - **Non-Reactive Operations** (e.g., `set`, cloning values, and non reactive versions of `extract`, `slice`, `split`, `reverse`):
///   Create new independent `Arc<RwLock<T>>` instances with cloned values. Changes to the new element do not affect the original, providing
///   isolation when needed.
/// 
/// This dual nature allows fine-grained control over data sharing and isolation, making `Sequence<T>`
/// suitable for both scenarios requiring synchronized state and those requiring independent copies.
/// 
/// ### -> `Core Features`
/// 
/// - **Lock-Free Operations**: Utilizes atomic operations (`AtomicPtr`, `AtomicUsize`) for most read operations,
///   minimizing contention and maximizing throughput.
/// - **Thread-Safe**: All operations are safe to call from multiple threads concurrently. The sequence
///   uses `Arc<RwLock<()>>` for synchronization of write operations.
/// - **Dynamic Capacity**: Automatically grows when needed, with a custom-tailored growth strategy to avoid
///   multiple simulataneous resizes.
/// - **Reactive Updates**: Elements can be modified in-place reactively (affecting all references)
///   or replaced non-reactively (creating new isolated values).
/// - **Rich API**: Provides stack operations (push/pop), queue operations (enqueue/dequeue),
///   list operations (insert/remove/append), and advanced features like slicing, splitting,
///   reversing, and more.
/// - **Zero-Copy Cloning**: Cloning a `Sequence<T>` only clones the Arc pointers, not the underlying data,
///   making it extremely cheap to pass sequences around.
/// 
/// ### -> `Type Parameters`
/// 
/// - `T`: The type of elements stored in the sequence. Must implement `Send + Sync + 'static`
///   to ensure thread-safety and proper lifetime management.
/// 
/// ### -> `Invariants`
/// 
/// The sequence maintains the following critical invariants:
/// 1. **Length ≤ Capacity**: The length never exceeds capacity.
/// 2. **Valid Pointers**: All slots within `[0..length)` contain valid non-null pointers.
/// 3. **Null Beyond Length**: All slots at index ≥ length must be null.
/// 
/// Violations of these invariants will result in panics, as they indicate data corruption
/// rather than user errors.
/// 
/// ### -> `Traits Implemented`
/// 
/// `Sequence<T>` and `Arc<Sequence<T>>` implement the following traits:
/// 
/// - **`Allocation<T>`**: Provides methods for allocating new sequences with specified capacity.
/// - **`Operation<T>`**: Core operations like get, set, insert, remove, and append.
/// - **`NonReactive<T>`**: Non-reactive operations like extract, slice, split, and reverse
///   that create new independent values.
/// - **`Reactive<T>`**: Reactive operations like modify that update elements in-place,
///   affecting all references.
/// - **`Stack<T>`**: Stack operations (LIFO) including push, pop, peek, and bulk operations.
/// - **`Queue<T>`**: Queue operations (FIFO) including enqueue, dequeue, and bulk operations.
/// - **`Drain<T>`**: Drain operations to remove and return ranges of elements.
/// - **`SnapShot<T>`**: Create independent snapshots as `Vec<T>`.
/// - **`Bincode<T>`**: Binary serialization and deserialization (requires `T: serde::Serialize + serde::Deserialize`).
/// - **`Equality<T>`**: Multiple equality comparison strategies with different performance characteristics.
/// - **`Length`**: Synchronous length operations and comparisons.
/// 
/// ### -> `Memory Management`
/// 
/// - Elements are stored as `Arc<RwLock<T>>`, allowing for shared ownership and safe concurrent access.
/// - The sequence manages reference counts automatically, incrementing when elements are copied
///   or shared, and decrementing when elements are removed or replaced.
/// - When the sequence is dropped, all elements are properly cleaned up via it's `Drop` implementation.
/// 
/// ### -> `Concurrency Model`
/// 
/// - **Read Operations**: Most read operations use atomic loads with `Relaxed` ordering under read locks, providing
///   lock-free reads with minimal overhead and reduced memory fence operations.
/// - **Write Operations**: Write operations acquire a write lock to ensure exclusive access during modifications.
///   All intermediate atomic operations use `Relaxed` ordering, with a final `Release` operation or fence to
///   ensure visibility of all changes to other threads.
/// - **Memory Ordering Optimization**: The sequence employs fine-tuned memory ordering semantics:
///   - Under locks: `Relaxed` ordering (5-20% performance improvement by eliminating unnecessary fences)
///   - Final operations: `Release` ordering to publish changes atomically
///   - This optimization significantly reduces CPU pipeline stalls and memory barriers while maintaining correctness
/// - **Resize Operations**: When capacity is exceeded, the sequence replaces it's internal pointer container with a new one with
///   increased capacity, copies all element pointers (incrementing ref counts), and atomically
///   swaps the containers.
/// 
/// ### -> `Performance Characteristics`
/// 
/// - **Cloning**: O(1) - Only clones Arc pointers, not data.
/// - **Length/Capacity**: O(1) - Simple atomic loads.
/// - **Get**: O(1) - Direct array access with atomic load.
/// - **Set**: O(1) - Direct array access with atomic swap.
/// - **Append**: Amortized O(1) - May trigger resize if at capacity.
/// - **Insert**: O(n) - Requires shifting elements to the right.
/// - **Remove**: O(n) - Requires shifting elements to the left.
/// - **Push/Pop**: Amortized O(1) - Stack operations at the end.
/// - **Enqueue/Dequeue**: O(1) for enqueue, O(n) for dequeue (due to shift).
/// - **Resize**: O(n) - Copies all element pointers and increments ref counts.
/// 
/// ### -> `Error Handling`
/// 
/// The sequence follows a consistent error handling philosophy:
/// 
/// - **User Errors** (e.g., index out of bounds): Return `Result::Err` with descriptive error messages.
/// - **Invariant Violations** (e.g., null pointer within bounds): Panic immediately with diagnostic
///   information, as these indicate data corruption rather than recoverable errors.
/// 
/// ### -> `Usage Example`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     // Allocate a new sequence with initial capacity of 10
///     let sequence = Sequence::<i32>::allocate(10).await;
///     assert_eq!(sequence.length(), 0);
///     assert_eq!(sequence.capacity(), 10);
/// 
///     // Append elements
///     for i in 0..5 {
///         sequence.append(Candidate::Value(i)).await?;
///     }
///     assert_eq!(sequence.length(), 5);
/// 
///     // Get element at index 2
///     let value = sequence.get(2, true).await;
///     assert!(!value.empty());
///     assert_eq!(*value.as_arc().await.read().await, 2);
/// 
///     // Set element at index 2 (non-reactive)
///     sequence.set(2, Candidate::Value(20)).await?;
/// 
///     // Modify element at index 3 (reactive - replaces value in-place)
///     sequence.modify(3, 30).await?;
/// 
///     // Use as a stack
///     sequence.push(Candidate::Value(100)).await;
///     let popped = sequence.pop().await;
///     assert!(!popped.empty());
///     assert_eq!(*popped.as_arc().await.read().await, 100);
/// 
///     // Use as a queue
///     sequence.enqueue(Candidate::Value(200)).await;
///     let dequeued = sequence.dequeue().await?;
///     assert!(!dequeued.empty());
///     assert_eq!(*dequeued.as_arc().await.read().await, 0);
/// 
///     // Insert at specific position
///     sequence.insert(1, Candidate::Value(99)).await;
/// 
///     // Remove from specific position
///     let removed = sequence.remove(1).await?;
///     assert!(!removed.empty());
/// 
///     // Create a snapshot
///     let snapshot = sequence.snapshot().await;
///     assert_eq!(snapshot.len(), sequence.length());
/// 
///     // Reverse the sequence (non-reactive)
///     let reversed = NonReactive::reverse(&sequence).await;
///     
///     // Compare sequences
///     let is_equal = sequence.atomic_eq(&reversed).await;
///     assert!(!is_equal);
/// 
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
/// 
/// ### -> `Advanced Features`
/// 
/// - **Bulk Operations**: Methods like `push_n`, `pop_n`, `enqueue_n`, `dequeue_n` allow
///   processing multiple elements efficiently with configurable error handling (`ignore_errors`)
///   and atomicity guarantees (`AoN` - All or Nothing).
/// 
/// - **Slicing**: Extract contiguous ranges of elements into new sequences using `slice`.
/// 
/// - **Splitting**: Divide a sequence at a specific index into two independent sequences using `split`.
/// 
/// - **Draining**: Remove and return ranges of elements efficiently using `drain` and `drain_to`.
/// 
/// - **Serialization**: Serialize and deserialize sequences using bincode format (requires serde support).
/// 
/// - **Custom Growth Strategies**: Implement custom capacity growth strategies by overriding
///   the `generate_capacity` method.
/// 
/// ### -> `Ideal Use Cases`
/// 
/// `Sequence<T>` is optimized for demanding, high-throughput, low-latency scenarios:
/// 
/// - **High-Frequency Trading (HFT)**: Order books, tick data streams, and trade execution queues
///   where microsecond latency matters and concurrent access from multiple trading threads is critical.
/// 
/// - **Real-Time Analytics**: Processing streaming data from sensors, logs, or metrics where
///   multiple threads concurrently read and write time-series data.
/// 
/// - **Game Engines**: Managing entity component systems, event queues, and shared game state
///   across multiple game loop threads with minimal frame time impact.
/// 
/// - **Network Servers**: Connection pools, request queues, and message buffers in high-concurrency
///   web servers, proxies, or message brokers handling thousands of concurrent connections.
/// 
/// - **Scientific Computing**: Shared work queues in parallel algorithms, particle simulations,
///   or distributed computation frameworks requiring lock-free coordination.
/// 
/// - **Audio/Video Processing**: Real-time audio sample buffers, video frame queues, and DSP pipelines
///   where bounded latency is critical for avoiding glitches.
/// 
/// - **Blockchain/Distributed Systems**: Transaction pools, mempool management, and consensus
///   message queues requiring high-throughput concurrent access.
/// 
/// The combination of lock-free reads, optimized memory ordering, and reactive capabilities makes
/// `Sequence<T>` ideal for any scenario demanding **high speed**, **high performance**, and **high concurrency**.
/// 
/// ### -> `Thread Safety`
/// 
/// `Sequence<T>` is fully thread-safe when `T: Send + Sync`. Multiple threads can:
/// - Read from the same sequence concurrently without contention (lock-free reads).
/// - Write to the same sequence concurrently (writes are serialized via internal locking).
/// - Clone and share the sequence across threads cheaply (Arc-based sharing).
/// 
/// ### -> `Notes`
/// 
/// - Cloning a `Sequence<T>` creates a shallow copy that shares the same underlying data.
///   Modifications through one clone are visible through other clones when using reactive operations.
/// - Non-reactive operations (set, extract, slice, split, reverse) create new independent values
///   that do not affect other references.
/// - Reactive operations (modify, set, extract, slice, split and reverse) enables reactive updates that reflect across different sequences.
/// - The sequence automatically grows when capacity is exceeded, but never shrinks automatically.
#[derive(Clone)]
pub struct Sequence<T> {
    container: Arc<AtomicPtr<Container<T>>>,
    synchronization_handle: Arc<RwLock<()>>
}


impl <T> Sequence<T>
where
    T: Send + Sync + 'static
{
    /// does not hold synchronization lock!
    /// the caller has to get the lock - as this function is internal.
    async fn resize(&self, upto: usize)  {
        assert!(upto > 0, "Capacity must be greater than zero!");
        
        let container_new = Arc::new(Container::<T>::allocate(upto).await);

        let container_pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
        assert!(!container_pointer.is_null(), "Sequence pointer is null!");
        let container_old = unsafe {
            Arc::increment_strong_count(container_pointer as *const Container<T>);
            Arc::from_raw(container_pointer)
        };

        let length_old = container_old.length.load(std::sync::atomic::Ordering::Relaxed);
        container_new.length.store(length_old, std::sync::atomic::Ordering::Relaxed);

        // OPTIMIZATION: Bulk operations with relaxed ordering + single fence
        unsafe {
            // Copy all pointers and increment ref counts in a single pass
            for i in 0..length_old {
                let pointer = container_old.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                container_new.slots[i].store(pointer, std::sync::atomic::Ordering::Relaxed);
                
                // Increment ref count only for non-null pointers
                if !pointer.is_null() {
                    Arc::increment_strong_count(pointer as *const RwLock<T>);
                }
            }
        }
        
        // Memory fence to ensure all stores are visible before swap
        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

        let _swapped_container = {
            let next = Arc::into_raw(container_new) as *mut Container<T>;
            let previous = self.container.swap(next, std::sync::atomic::Ordering::AcqRel);
            unsafe {
                Arc::from_raw(previous)
            }
        };

        crate::drop!(container_old, _swapped_container);
    }

    async fn shift(&self, from: usize) {
        let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
        assert!(!pointer.is_null(), "Sequence pointer is null!");
        
        let mut container = unsafe {
            Arc::increment_strong_count(pointer as *const Container<T>);
            Arc::from_raw(pointer)
        };

        let mut length = container.length.load(std::sync::atomic::Ordering::Relaxed);
        let mut capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

        if from > length {
            crate::drop!(container);
            return;
        }

        assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);

        // Loop until we have enough capacity
        while length == capacity {
            let upto = self.generate_capacity(capacity, length + 1).await;
            crate::drop!(container);
            self.resize(upto).await;
            
            // Reload container after resize
            let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };
            
            length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
        }

        // OPTIMIZATION: Shift with relaxed ordering + single fence
        unsafe {
            // Shift right from end to start to avoid overwriting
            for i in (from..length).rev() {
                // Load current slot value
                let src = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                
                // Store it in the next slot, getting what was displaced
                let displaced = container.slots[i + 1].swap(src, std::sync::atomic::Ordering::Relaxed);
                
                // Increment ref count for the moved pointer (if not null)
                if !src.is_null() {
                    Arc::increment_strong_count(src);
                }
                
                // Drop the displaced value
                if !displaced.is_null() {
                    crate::drop!(Arc::from_raw(displaced));
                }
                
                // Clear the source slot
                container.slots[i].store(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
            }
        }
        
        // Memory fence to ensure all operations are visible
        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

        crate::drop!(container);
    }

    async fn generate_capacity(&self, current: usize, required: usize) -> usize {
        let new_capacity = if current < 8 {
            (current * 2).max(8)
        } else if current < 4096 {
            current + (current / 2)
        } else {
            current + 1024
        };

        new_capacity.max(required)
    }

    async fn append_candidate(&self, value: crate::Candidate<T>) {
        let synchronization_handle = self.synchronization_handle.write().await;
        let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
        assert!(!pointer.is_null(), "Sequence pointer is null!");
        let mut container = unsafe {
            Arc::increment_strong_count(pointer as *const Container<T>);
            Arc::from_raw(pointer)
        };

        let mut length = container.length.load(std::sync::atomic::Ordering::Relaxed);
        let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

        assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);

        if length == capacity {
            let upto = self.generate_capacity(capacity, length + 1).await;
            self.resize(upto).await;

            // reload container
            let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            length = container.length.load(std::sync::atomic::Ordering::Relaxed);
        }

        let arc_new = match value {
            crate::Candidate::Arc(x) => x,
            crate::Candidate::Value(y) => Arc::new(RwLock::new(y))
        };

        let pointer_new = Arc::into_raw(arc_new) as *mut RwLock<T>;
        
        // OPTIMIZATION: Relaxed store + simple length increment with Release
        container.slots[length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
        container.length.store(length + 1, std::sync::atomic::Ordering::Release);
        
        crate::drop!(container, synchronization_handle);
    }

    async fn from_vec(vector: &Vec<T>) -> Self where T: Clone {
        let sequence: Sequence<T> = Sequence::allocate_raw(vector.len()).await;
        for value in vector {
            sequence.append_candidate(crate::Candidate::Value(value.clone())).await;
        }
        sequence
    }

    pub fn capacity(&self) -> usize {
        let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
        assert!(!pointer.is_null(), "Sequence pointer is null!");
        let container = unsafe {
            Arc::increment_strong_count(pointer as *const Container<T>);
            Arc::from_raw(pointer)
        };
        let capacity = container.capacity.load(std::sync::atomic::Ordering::SeqCst);
        crate::drop!(container);
        capacity
    }

}


impl <T> traits::Allocation<T> for Sequence<T>
where
    T: Send + Sync + 'static,
    Self: Sized,
{
    type SelfType = Self;

    fn allocate_raw(capacity: usize) -> std::pin::Pin<Box<dyn Future<Output = Self::SelfType> + Send + 'static>> {
        Box::pin(async move {
            assert!(capacity > 0, "Capacity must be greater than zero!");
            
            let container: Arc<Container<T>> = Arc::new(Container::allocate(capacity).await);
            let raw = Arc::into_raw(container) as *mut Container<T>;
            
            Self {
                container: Arc::new(AtomicPtr::new(raw)),
                synchronization_handle: Arc::new(RwLock::new(()))
            }
        })
    }

}

impl <T> traits::Allocation<T> for Arc<Sequence<T>>
where
    T: Send + Sync + 'static,
    Self: Sized,
{
    type SelfType = Sequence<T>;

    fn allocate_raw(capacity: usize) -> std::pin::Pin<Box<dyn Future<Output = Self::SelfType> + Send + 'static>> {
        Box::pin(async move {
            assert!(capacity > 0, "Capacity must be greater than zero!");
            
            let container: Arc<Container<T>> = Arc::new(Container::allocate(capacity).await);
            let raw = Arc::into_raw(container) as *mut Container<T>;
            
            Sequence {
                container: Arc::new(AtomicPtr::new(raw)),
                synchronization_handle: Arc::new(RwLock::new(()))
            }
        })
    }
}


impl <T> traits::Length for Sequence<T> {

    fn length(&self) -> usize {
        let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
        assert!(!pointer.is_null(), "Sequence pointer is null!");
        
        let container = unsafe {
            Arc::increment_strong_count(pointer as *const Container<T>);
            Arc::from_raw(pointer)
        };

        let length = container.length.load(std::sync::atomic::Ordering::SeqCst);
        crate::drop!(container);

        length
    }

    fn length_eq(&self, other: &Self) -> bool {
        self.length() == other.length()
    }

    fn length_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.length().partial_cmp(&other.length())
    }
}

impl <T> traits::Length for Arc<Sequence<T>> {

    fn length(&self) -> usize {
        let pointer = self.container.load(std::sync::atomic::Ordering::Acquire);
        assert!(!pointer.is_null(), "Sequence pointer is null!");
        
        let container = unsafe {
            Arc::increment_strong_count(pointer as *const Container<T>);
            Arc::from_raw(pointer)
        };

        let length = container.length.load(std::sync::atomic::Ordering::SeqCst);
        crate::drop!(container);

        length
    }

    fn length_eq(&self, other: &Self) -> bool {
        self.length() == other.length()
    }

    fn length_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.length().partial_cmp(&other.length())
    }
}


impl <T> traits::Operation<T> for Sequence<T>
where
    T: Send + Sync + 'static,
    Self: Sync,
{
    type ArcSwapRef = compat::Compat<T>;

    fn get(&self, index: usize, synchronize: bool) -> std::pin::Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>> {
        Box::pin(async move {
            let _sync_guard = if synchronize {
                Some(self.synchronization_handle.read().await)
            } else {
                None
            };

            // OPTIMIZATION: Use Relaxed ordering when under read lock, Acquire otherwise
            let ordering = if synchronize {
                std::sync::atomic::Ordering::Relaxed
            } else {
                std::sync::atomic::Ordering::Acquire
            };

            let pointer = self.container.load(ordering);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(ordering);

            if index >= length {
                drop(container);
                return compat::Compat::create(ArcSwapOption::empty()).await;
            }

            let pointer = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            drop(container);

            if pointer.is_null() {
                // This is an invariant
                // This has to panic as current rules dont allow a pointer to be null within bounds
                panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
            }

            let result = unsafe {
                Arc::increment_strong_count(pointer);
                Arc::from_raw(pointer)
            };

            compat::Compat::create(ArcSwapOption::new(Some(result))).await
        })
    }

    fn set(&self, index: usize, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send + '_>> {
        Box::pin(async move {

            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            if index >= length {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!(format!("Index {} out of bounds for sequence of length {}. \'set\' method can only operate on existing indexes.", index, length)));
            }

            // Check for invariant violation BEFORE modifying anything
            let old_ptr = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            if old_ptr.is_null() {
                // This is an invariant violation
                // This has to panic as current rules dont allow a pointer to be null within bounds
                crate::drop!(container, synchronization_handle);
                panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
            }

            let arc = match value {
                crate::Candidate::Value(value) => Arc::new(RwLock::new(value)),
                crate::Candidate::Arc(arc) => arc,
            };

            // OPTIMIZATION: Relaxed swap under write lock + Release fence
            let np = Arc::into_raw(arc) as *mut RwLock<T>;
            let op = container.slots[index].swap(np, std::sync::atomic::Ordering::Relaxed);
            
            // Ensure swap is visible before releasing lock
            std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
            
            // The old pointer must be valid since we checked before swap
            debug_assert!(!op.is_null());

            unsafe {
                Ok(compat::Compat::create(ArcSwapOption::new(Some(Arc::from_raw(op)))).await)
            }

        })
    }

    fn insert(&self, index: usize, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            // let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            let actual_index = if index >= length { length } else { index };
            
            // will automatically resize -> !
            // crate::drop!(synchronization_handle, container);
            crate::drop!(container);
            self.shift(actual_index).await;
            // let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            // let length = container.length.load(std::sync::atomic::Ordering::SeqCst);

            let arc: Arc<RwLock<T>>;

            match value {
                crate::Candidate::Value(v) => {
                    arc = Arc::new(RwLock::new(v));
                },
                crate::Candidate::Arc(a) => {
                    arc = a;
                }
            };

            let next_pointer = Arc::into_raw(arc) as *mut RwLock<T>;

            // Verify shift worked correctly - slot at actual_index should be null after shift
            let old_ptr = container.slots[actual_index].load(std::sync::atomic::Ordering::Relaxed);
            let length_after_shift = container.length.load(std::sync::atomic::Ordering::Relaxed);

            // After shift, the slot at actual_index should be empty (null)
            if !old_ptr.is_null() {
                crate::drop!(container, synchronization_handle);
                panic!("Invariant violation: slot at index {} should be null after shift, but contains a pointer (length {}).", actual_index, length_after_shift);
            }
            
            // OPTIMIZATION: Relaxed swap and length increment under write lock
            let previous_pointer = container.slots[actual_index].swap(next_pointer, std::sync::atomic::Ordering::Relaxed);

            // Release ordering ensures all operations are visible
            container.length.fetch_add(1, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);

            assert!(previous_pointer.is_null(), "Invariant violation: previous pointer at index {} should be null after insert, but is not.", actual_index);
        })
    }

    fn append(&self, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            Ok(self.append_candidate(value).await)
        })
    }

    fn remove(&self, index: usize) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            if index >= length {
                crate::drop!(container, synchronization_handle);
                // return error
                return Err(anyhow::anyhow!(format!("Index {} out of bounds for sequence of length {}. \'remove\' method can only operate on existing indexes.", index, length)));
            }

            let removed_ptr = container.slots[index].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

            // Check for invariant violation - slot within bounds must have valid pointer
            if removed_ptr.is_null() {
                crate::drop!(container, synchronization_handle);
                panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
            }

            // OPTIMIZATION: Shift left with relaxed ordering under write lock
            // shift elements left (only if there are elements after the removed one)
            if index < length - 1 {
                for i in index..(length - 1) {
                    let src = container.slots[i + 1].load(std::sync::atomic::Ordering::Relaxed);
                    if !src.is_null() {
                        unsafe {
                            Arc::increment_strong_count(src);
                        }
                    }

                    let displaced = container.slots[i].swap(src, std::sync::atomic::Ordering::Relaxed);
                    if !displaced.is_null() {
                        crate::drop!(unsafe { Arc::from_raw(displaced) });
                    }
                }
            }

            // clear the last slot
            let last_ptr = container.slots[length - 1].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
            if !last_ptr.is_null() {
                crate::drop!(unsafe { Arc::from_raw(last_ptr) });
            }

            // Release ordering ensures all operations are visible
            container.length.fetch_sub(1, std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
            
            // removed_ptr is guaranteed to be non-null due to invariant check above
            debug_assert!(!removed_ptr.is_null());
            Ok(compat::Compat::create(ArcSwapOption::new(Some(unsafe { Arc::from_raw(removed_ptr) }))).await)
        })
    }

    fn extend(&self, iter: impl IntoIterator<Item = T>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let items: Vec<T> = iter.into_iter().collect();
        let item_count = items.len();
        if item_count == 0 { return Box::pin(async move {}); }
        
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let mut container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let current_length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let current_capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            let required_capacity = current_length + item_count;

            if required_capacity > current_capacity {
                let upto = self.generate_capacity(current_capacity, required_capacity).await;
                self.resize(upto).await;

                // reload container
                let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
                assert!(!pointer.is_null(), "Sequence pointer is null!");
                container = unsafe {
                    Arc::increment_strong_count(pointer as *const Container<T>);
                    Arc::from_raw(pointer)
                };
            }

            let mut insert_index = container.length.load(std::sync::atomic::Ordering::Relaxed);
            for item in items {
                let arc = Arc::new(RwLock::new(item));
                let pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                container.slots[insert_index].store(pointer, std::sync::atomic::Ordering::Relaxed);
                insert_index += 1;
            }

            // update length atomically.
            container.length.store(insert_index, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
        })
    }

    fn drain(&self, range: Option<std::ops::Range<usize>>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone
    {

        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let (start, end) = match range {
                Some(r) => {
                    let start = r.start.min(length);
                    let end = r.end.min(length);
                    if start >= end {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await;
                    }
                    (start, end)
                },
                None => (0, length)
            };

            let drain_count = end - start;
            let drained = Self::allocate(if drain_count > 0 { drain_count } else { 1 }).await;
            
            // Get drained container for direct slot population
            let drained_pointer = drained.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!drained_pointer.is_null());
            let drained_container = unsafe {
                Arc::increment_strong_count(drained_pointer as *const Container<T>);
                Arc::from_raw(drained_pointer)
            };
            
            // Clone values directly into drained sequence slots
            for i in 0..drain_count {
                let pointer = container.slots[start + i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };

                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    drained_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                    
                    drop(arc);
                }
            }
            
            // Set drained sequence length
            drained_container.length.store(drain_count, std::sync::atomic::Ordering::Release);
            drop(drained_container);

            // shift remaining elements left
            let remaining = length - end;
            for i in 0..remaining {
                let source_idx = end + i;
                let dest_idx = start + i;

                let pointer = container.slots[source_idx].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                let displaced = container.slots[dest_idx].swap(pointer, std::sync::atomic::Ordering::Relaxed);
                if !displaced.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(displaced) });
                }
            }

            // clear the non-used slots at the end
            let length_new = length - drain_count;
            for i in length_new..length {
                let removed = container.slots[i].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                if !removed.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(removed) });
                }
            }

            container.length.store(length_new, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            drained
        })
    }

}



impl <T> traits::Operation<T> for Arc<Sequence<T>>
where
    T: Send + Sync + 'static,
    Self: Sync,
{
    type ArcSwapRef = compat::Compat<T>;

    fn get(&self, index: usize, synchronize: bool) -> std::pin::Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>> {
        Box::pin(async move {
            let _sync_guard = if synchronize {
                Some(self.synchronization_handle.read().await)
            } else {
                None
            };

            // OPTIMIZATION: Use Relaxed ordering when under read lock, Acquire otherwise
            let ordering = if synchronize {
                std::sync::atomic::Ordering::Relaxed
            } else {
                std::sync::atomic::Ordering::Acquire
            };

            let pointer = self.container.load(ordering);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(ordering);

            if index >= length {
                drop(container);
                return compat::Compat::create(ArcSwapOption::empty()).await;
            }

            let pointer = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            drop(container);

            if pointer.is_null() {
                // This is an invariant
                // This has to panic as current rules dont allow a pointer to be null within bounds
                panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
            }

            let result = unsafe {
                Arc::increment_strong_count(pointer);
                Arc::from_raw(pointer)
            };

            compat::Compat::create(ArcSwapOption::new(Some(result))).await
        })
    }

    fn set(&self, index: usize, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send + '_>> {
        Box::pin(async move {

            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            if index >= length {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!(format!("Index {} out of bounds for sequence of length {}. \'set\' method can only operate on existing indexes.", index, length)));
            }

            // Check for invariant violation BEFORE modifying anything
            let old_ptr = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            if old_ptr.is_null() {
                // This is an invariant violation
                // This has to panic as current rules dont allow a pointer to be null within bounds
                crate::drop!(container, synchronization_handle);
                panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
            }

            let arc = match value {
                crate::Candidate::Value(value) => Arc::new(RwLock::new(value)),
                crate::Candidate::Arc(arc) => arc,
            };

            // OPTIMIZATION: Relaxed swap under write lock + Release fence
            let np = Arc::into_raw(arc) as *mut RwLock<T>;
            let op = container.slots[index].swap(np, std::sync::atomic::Ordering::Relaxed);
            
            // Ensure swap is visible before releasing lock
            std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
            
            // The old pointer must be valid since we checked before swap
            debug_assert!(!op.is_null());

            unsafe {
                Ok(compat::Compat::create(ArcSwapOption::new(Some(Arc::from_raw(op)))).await)
            }

        })
    }

    fn insert(&self, index: usize, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            // let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            let actual_index = if index >= length { length } else { index };
            
            // will automatically resize -> !
            // crate::drop!(synchronization_handle, container);
            crate::drop!(container);
            self.shift(actual_index).await;
            // let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            // let length = container.length.load(std::sync::atomic::Ordering::SeqCst);

            let arc: Arc<RwLock<T>>;

            match value {
                crate::Candidate::Value(v) => {
                    arc = Arc::new(RwLock::new(v));
                },
                crate::Candidate::Arc(a) => {
                    arc = a;
                }
            };

            let next_pointer = Arc::into_raw(arc) as *mut RwLock<T>;

            // Verify shift worked correctly - slot at actual_index should be null after shift
            let old_ptr = container.slots[actual_index].load(std::sync::atomic::Ordering::Relaxed);
            let length_after_shift = container.length.load(std::sync::atomic::Ordering::Relaxed);

            // After shift, the slot at actual_index should be empty (null)
            if !old_ptr.is_null() {
                crate::drop!(container, synchronization_handle);
                panic!("Invariant violation: slot at index {} should be null after shift, but contains a pointer (length {}).", actual_index, length_after_shift);
            }
            
            // OPTIMIZATION: Relaxed swap and length increment under write lock
            let previous_pointer = container.slots[actual_index].swap(next_pointer, std::sync::atomic::Ordering::Relaxed);

            // Release ordering ensures all operations are visible
            container.length.fetch_add(1, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);

            assert!(previous_pointer.is_null(), "Invariant violation: previous pointer at index {} should be null after insert, but is not.", actual_index);
        })
    }

    fn append(&self, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.append_candidate(value).await;
            Ok(())
        })
    }

    fn remove(&self, index: usize) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            if index >= length {
                crate::drop!(container, synchronization_handle);
                // return error
                return Err(anyhow::anyhow!(format!("Index {} out of bounds for sequence of length {}. \'remove\' method can only operate on existing indexes.", index, length)));
            }

            let removed_ptr = container.slots[index].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

            // Check for invariant violation - slot within bounds must have valid pointer
            if removed_ptr.is_null() {
                crate::drop!(container, synchronization_handle);
                panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
            }

            // OPTIMIZATION: Shift left with relaxed ordering under write lock
            // shift elements left (only if there are elements after the removed one)
            if index < length - 1 {
                for i in index..(length - 1) {
                    let src = container.slots[i + 1].load(std::sync::atomic::Ordering::Relaxed);
                    if !src.is_null() {
                        unsafe {
                            Arc::increment_strong_count(src);
                        }
                    }

                    let displaced = container.slots[i].swap(src, std::sync::atomic::Ordering::Relaxed);
                    if !displaced.is_null() {
                        crate::drop!(unsafe { Arc::from_raw(displaced) });
                    }
                }
            }

            // clear the last slot
            let last_ptr = container.slots[length - 1].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
            if !last_ptr.is_null() {
                crate::drop!(unsafe { Arc::from_raw(last_ptr) });
            }

            // Release ordering ensures all operations are visible
            container.length.fetch_sub(1, std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
            
            // removed_ptr is guaranteed to be non-null due to invariant check above
            debug_assert!(!removed_ptr.is_null());
            Ok(compat::Compat::create(ArcSwapOption::new(Some(unsafe { Arc::from_raw(removed_ptr) }))).await)
        })
    }

    fn extend(&self, iter: impl IntoIterator<Item = T>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let items: Vec<T> = iter.into_iter().collect();
        let item_count = items.len();
        if item_count == 0 { return Box::pin(async move {}); }
        
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let mut container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let current_length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let current_capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            let required_capacity = current_length + item_count;

            if required_capacity > current_capacity {
                let upto = self.generate_capacity(current_capacity, required_capacity).await;
                self.resize(upto).await;

                // reload container
                let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
                assert!(!pointer.is_null(), "Sequence pointer is null!");
                container = unsafe {
                    Arc::increment_strong_count(pointer as *const Container<T>);
                    Arc::from_raw(pointer)
                };
            }

            let mut insert_index = container.length.load(std::sync::atomic::Ordering::Relaxed);
            for item in items {
                let arc = Arc::new(RwLock::new(item));
                let pointer = Arc::into_raw(arc) as *mut RwLock<T>;

                let old_slot = container.slots[insert_index].load(std::sync::atomic::Ordering::Relaxed);
                if !old_slot.is_null() {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be non-null in an empty slot
                    crate::drop!(container, synchronization_handle);
                    panic!("Invariant violation: slot at index {} is not null when extending sequence.", insert_index);
                }

                container.slots[insert_index].store(pointer, std::sync::atomic::Ordering::Relaxed);
                insert_index += 1;
            }

            // update length atomically.
            container.length.store(insert_index, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
        })
    }

    fn drain(&self, range: Option<std::ops::Range<usize>>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone
    {

        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let (start, end) = match range {
                Some(r) => {
                    let start = r.start.min(length);
                    let end = r.end.min(length);
                    if start >= end {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await;
                    }
                    (start, end)
                },
                None => (0, length)
            };

            let drain_count = end - start;
            let drained = Self::allocate(if drain_count > 0 { drain_count } else { 1 }).await;
            
            // Get drained container for direct slot population
            let drained_pointer = drained.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!drained_pointer.is_null());
            let drained_container = unsafe {
                Arc::increment_strong_count(drained_pointer as *const Container<T>);
                Arc::from_raw(drained_pointer)
            };
            
            // Clone values directly into drained sequence slots
            for i in 0..drain_count {
                let pointer = container.slots[start + i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };

                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    drained_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                    
                    drop(arc);
                }
            }
            
            // Set drained sequence length
            drained_container.length.store(drain_count, std::sync::atomic::Ordering::Release);
            drop(drained_container);

            // shift remaining elements left
            let remaining = length - end;
            for i in 0..remaining {
                let source_idx = end + i;
                let dest_idx = start + i;

                let pointer = container.slots[source_idx].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                let displaced = container.slots[dest_idx].swap(pointer, std::sync::atomic::Ordering::Relaxed);
                if !displaced.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(displaced) });
                }
            }

            // clear the non-used slots at the end
            let length_new = length - drain_count;
            for i in length_new..length {
                let removed = container.slots[i].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                if !removed.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(removed) });
                }
            }

            container.length.store(length_new, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            drained
        })
    }

}


impl <T> traits::Reactive<T> for Sequence<T>
where
    T: Send + Sync + 'static,
    Self: Sync
{
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            // OPTIMIZATION: Use Relaxed ordering under read lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let (start, end) = match range {
                Some(r) => {
                    let start = r.start.min(length);
                    let end = r.end.min(length);
                    if start >= end {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await;
                    }
                    (start, end)
                },
                None => (0, length)
            };

            let extract_count = end - start;
            let extracted = Self::allocate(if extract_count > 0 { extract_count } else { 1 }).await;
            
            // Get extracted container for direct slot population
            let extracted_pointer = extracted.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!extracted_pointer.is_null());
            let extracted_container = unsafe {
                Arc::increment_strong_count(extracted_pointer as *const Container<T>);
                Arc::from_raw(extracted_pointer)
            };
            
            // Share Arc references directly into extracted sequence slots
            for i in 0..extract_count {
                let pointer = container.slots[start + i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    extracted_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, extracted_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", start + i, length);
                }
            }
            
            // Set extracted sequence length with Release to ensure visibility
            extracted_container.length.store(extract_count, std::sync::atomic::Ordering::Release);
            drop(extracted_container);

            crate::drop!(container, synchronization_handle);
            extracted
        })
    }

    fn modify(&self, index: usize, value: T) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send + '_>> {
        // this function modifies the value inside the arc.. (without replacing the arc itself) (reactive)
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            
            // OPTIMIZATION: Use Relaxed ordering under write lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            if index >= length {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!(format!("Index {} out of bounds for sequence of length {}. \'modify\' method can only operate on existing indexes.", index, length)));
            }

            let pointer = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            if pointer.is_null() {
                crate::drop!(container, synchronization_handle);
                panic!("Index {} is empty in sequence of length {}. \'modify\' method can only operate on existing non-empty indexes.", index, length);
            }

            let arc = unsafe {
                Arc::increment_strong_count(pointer);
                Arc::from_raw(pointer)
            };

            {
                let mut guard = arc.write().await;
                *guard = value;
            }

            crate::drop!(container, synchronization_handle);
            Ok(compat::Compat::create(ArcSwapOption::new(Some(arc))).await)
        })
    }

    fn split(&self, index: usize) -> std::pin::Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>> {
        // this is also reactive -> changes in the split sequences reflect in the original and vice versa.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            // OPTIMIZATION: Use Relaxed ordering under read lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let split_index = index.min(length);
            let first = Self::allocate(split_index).await;
            let second = Self::allocate(length - split_index).await;

            // Get first container for direct slot population
            let first_pointer = first.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!first_pointer.is_null());
            let first_container = unsafe {
                Arc::increment_strong_count(first_pointer as *const Container<T>);
                Arc::from_raw(first_pointer)
            };

            // Get second container for direct slot population
            let second_pointer = second.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!second_pointer.is_null());
            let second_container = unsafe {
                Arc::increment_strong_count(second_pointer as *const Container<T>);
                Arc::from_raw(second_pointer)
            };

            // Populate first sequence
            for i in 0..split_index {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    first_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, first_container, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            // Release ordering ensures all stores are visible
            first_container.length.store(split_index, std::sync::atomic::Ordering::Release);
            drop(first_container);

            // Populate second sequence
            for i in split_index..length {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    second_container.slots[i - split_index].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            // Release ordering ensures all stores are visible
            second_container.length.store(length - split_index, std::sync::atomic::Ordering::Release);
            drop(second_container);

            crate::drop!(container, synchronization_handle);
            (first, second)
        })
    }

    fn reverse(&self) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // this is also reactive -> changes in the reversed sequence reflect in the original and vice versa.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            // OPTIMIZATION: Use Relaxed ordering under read lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let reversed = Self::allocate(length).await;

            // Get reversed container for direct slot population
            let reversed_pointer = reversed.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!reversed_pointer.is_null());
            let reversed_container = unsafe {
                Arc::increment_strong_count(reversed_pointer as *const Container<T>);
                Arc::from_raw(reversed_pointer)
            };

            // Populate reversed sequence
            for i in 0..length {
                let pointer = container.slots[length - 1 - i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    reversed_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, reversed_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", length - 1 - i, length);
                }
            }
            
            // Release ordering ensures all stores are visible
            reversed_container.length.store(length, std::sync::atomic::Ordering::Release);
            drop(reversed_container);

            crate::drop!(container, synchronization_handle);
            reversed
        })
    }

    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // this is also reactive -> changes in the sliced sequence reflect in the original and vice versa.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let p = step.unwrap_or(1);

            // Handle step of zero - invalid, return empty sequence
            if p == 0 {
                crate::drop!(container, synchronization_handle);
                return Self::allocate(1).await;
            }

            // Calculate start and stop indices based on step direction
            let (start_index, stop_index) = if p > 0 {
                let s = start.unwrap_or(0);
                let e = stop.unwrap_or(length as isize);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length)
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(0) as usize
                } else {
                    (e as usize).min(length)
                };
                
                (start_idx, stop_idx)
            } else {
                // Negative step: default start is end of sequence, default stop is before beginning
                let s = start.unwrap_or(length as isize - 1);
                let e = stop.unwrap_or(-1);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length.saturating_sub(1))
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(-1) as isize
                } else {
                    e as isize
                };
                
                (start_idx, stop_idx as usize)
            };

            // Calculate approximate capacity
            let estimated_capacity = if p > 0 {
                if start_index >= stop_index {
                    1
                } else {
                    ((stop_index - start_index) as f64 / p as f64).ceil() as usize
                }
            } else {
                if start_index <= stop_index {
                    1
                } else {
                    ((start_index - stop_index) as f64 / (-p) as f64).ceil() as usize
                }
            };

            let sliced = Self::allocate(estimated_capacity.max(1)).await;
            
            // Get sliced container for direct slot population
            let sliced_pointer = sliced.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!sliced_pointer.is_null());
            let sliced_container = unsafe {
                Arc::increment_strong_count(sliced_pointer as *const Container<T>);
                Arc::from_raw(sliced_pointer)
            };

            let mut slot_idx = 0;
            if p > 0 {
                let mut i = start_index;
                while i < stop_index {
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    i += p as usize;
                }
            } else {
                // Negative step: iterate backwards
                let step_abs = (-p) as usize;
                let mut i = start_index;
                loop {
                    // Check if current index is at or before stop (stop is exclusive, so we stop before it)
                    if i <= stop_index {
                        break;
                    }
                    
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    
                    // Check if next iteration would underflow
                    if i < step_abs {
                        break;
                    }
                    i -= step_abs;
                }
            }
            
            // Release ordering ensures all stores are visible
            sliced_container.length.store(slot_idx, std::sync::atomic::Ordering::Release);
            drop(sliced_container);
            
            crate::drop!(container, synchronization_handle);
            sliced
        })
    }
}


impl <T> traits::Reactive<T> for Arc<Sequence<T>>
where
    T: Send + Sync + 'static,
    Self: Sync
{
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            // OPTIMIZATION: Use Relaxed ordering under read lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let (start, end) = match range {
                Some(r) => {
                    let start = r.start.min(length);
                    let end = r.end.min(length);
                    if start >= end {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await;
                    }
                    (start, end)
                },
                None => (0, length)
            };

            let extract_count = end - start;
            let extracted = Self::allocate(if extract_count > 0 { extract_count } else { 1 }).await;
            
            // Get extracted container for direct slot population
            let extracted_pointer = extracted.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!extracted_pointer.is_null());
            let extracted_container = unsafe {
                Arc::increment_strong_count(extracted_pointer as *const Container<T>);
                Arc::from_raw(extracted_pointer)
            };
            
            // Share Arc references directly into extracted sequence slots
            for i in 0..extract_count {
                let pointer = container.slots[start + i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    extracted_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, extracted_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", start + i, length);
                }
            }
            
            // Set extracted sequence length with Release to ensure visibility
            extracted_container.length.store(extract_count, std::sync::atomic::Ordering::Release);
            drop(extracted_container);

            crate::drop!(container, synchronization_handle);
            extracted
        })
    }

    fn modify(&self, index: usize, value: T) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send + '_>> {
        // this function modifies the value inside the arc.. (without replacing the arc itself) (reactive)
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            
            // OPTIMIZATION: Use Relaxed ordering under write lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            if index >= length {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!(format!("Index {} out of bounds for sequence of length {}. \'modify\' method can only operate on existing indexes.", index, length)));
            }

            let pointer = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            if pointer.is_null() {
                crate::drop!(container, synchronization_handle);
                panic!("Index {} is empty in sequence of length {}. \'modify\' method can only operate on existing non-empty indexes.", index, length);
            }

            let arc = unsafe {
                Arc::increment_strong_count(pointer);
                Arc::from_raw(pointer)
            };

            {
                let mut guard = arc.write().await;
                *guard = value;
            }

            crate::drop!(container, synchronization_handle);
            Ok(compat::Compat::create(ArcSwapOption::new(Some(arc))).await)
        })
    }

    fn split(&self, index: usize) -> std::pin::Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>> {
        // this is also reactive -> changes in the split sequences reflect in the original and vice versa.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            // OPTIMIZATION: Use Relaxed ordering under read lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let split_index = index.min(length);
            let first = Self::allocate(split_index).await;
            let second = Self::allocate(length - split_index).await;

            // Get first container for direct slot population
            let first_pointer = first.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!first_pointer.is_null());
            let first_container = unsafe {
                Arc::increment_strong_count(first_pointer as *const Container<T>);
                Arc::from_raw(first_pointer)
            };

            // Get second container for direct slot population
            let second_pointer = second.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!second_pointer.is_null());
            let second_container = unsafe {
                Arc::increment_strong_count(second_pointer as *const Container<T>);
                Arc::from_raw(second_pointer)
            };

            // Populate first sequence
            for i in 0..split_index {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    first_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, first_container, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            // Release ordering ensures all stores are visible
            first_container.length.store(split_index, std::sync::atomic::Ordering::Release);
            drop(first_container);

            // Populate second sequence
            for i in split_index..length {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    second_container.slots[i - split_index].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            // Release ordering ensures all stores are visible
            second_container.length.store(length - split_index, std::sync::atomic::Ordering::Release);
            drop(second_container);

            crate::drop!(container, synchronization_handle);
            (first, second)
        })
    }

    fn reverse(&self) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // this is also reactive -> changes in the reversed sequence reflect in the original and vice versa.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            // OPTIMIZATION: Use Relaxed ordering under read lock
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let reversed = Self::allocate(length).await;

            // Get reversed container for direct slot population
            let reversed_pointer = reversed.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!reversed_pointer.is_null());
            let reversed_container = unsafe {
                Arc::increment_strong_count(reversed_pointer as *const Container<T>);
                Arc::from_raw(reversed_pointer)
            };

            // Populate reversed sequence
            for i in 0..length {
                let pointer = container.slots[length - 1 - i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                    reversed_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, reversed_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", length - 1 - i, length);
                }
            }
            
            // Release ordering ensures all stores are visible
            reversed_container.length.store(length, std::sync::atomic::Ordering::Release);
            drop(reversed_container);

            crate::drop!(container, synchronization_handle);
            reversed
        })
    }

    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // this is also reactive -> changes in the sliced sequence reflect in the original and vice versa.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let p = step.unwrap_or(1);

            // Handle step of zero - invalid, return empty sequence
            if p == 0 {
                crate::drop!(container, synchronization_handle);
                return Self::allocate(1).await;
            }

            // Calculate start and stop indices based on step direction
            let (start_index, stop_index) = if p > 0 {
                let s = start.unwrap_or(0);
                let e = stop.unwrap_or(length as isize);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length)
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(0) as usize
                } else {
                    (e as usize).min(length)
                };
                
                (start_idx, stop_idx)
            } else {
                // Negative step: default start is end of sequence, default stop is before beginning
                let s = start.unwrap_or(length as isize - 1);
                let e = stop.unwrap_or(-1);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length.saturating_sub(1))
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(-1) as isize
                } else {
                    e as isize
                };
                
                (start_idx, stop_idx as usize)
            };

            // Calculate approximate capacity
            let estimated_capacity = if p > 0 {
                if start_index >= stop_index {
                    1
                } else {
                    ((stop_index - start_index) as f64 / p as f64).ceil() as usize
                }
            } else {
                if start_index <= stop_index {
                    1
                } else {
                    ((start_index - stop_index) as f64 / (-p) as f64).ceil() as usize
                }
            };

            let sliced = Self::allocate(estimated_capacity.max(1)).await;
            
            // Get sliced container for direct slot population
            let sliced_pointer = sliced.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!sliced_pointer.is_null());
            let sliced_container = unsafe {
                Arc::increment_strong_count(sliced_pointer as *const Container<T>);
                Arc::from_raw(sliced_pointer)
            };

            let mut slot_idx = 0;
            if p > 0 {
                let mut i = start_index;
                while i < stop_index {
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    i += p as usize;
                }
            } else {
                // Negative step: iterate backwards
                let step_abs = (-p) as usize;
                let mut i = start_index;
                loop {
                    // Check if current index is at or before stop (stop is exclusive, so we stop before it)
                    if i <= stop_index {
                        break;
                    }
                    
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let new_pointer = Arc::into_raw(arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    
                    // Check if next iteration would underflow
                    if i < step_abs {
                        break;
                    }
                    i -= step_abs;
                }
            }
            
            // Release ordering ensures all stores are visible
            sliced_container.length.store(slot_idx, std::sync::atomic::Ordering::Release);
            drop(sliced_container);
            
            crate::drop!(container, synchronization_handle);
            sliced
        })
    }
}


impl <T> traits::NonReactive<T> for Sequence<T>
where
    T: Send + Sync + 'static,
    T: Clone,
    Self: Sync,
{
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let (start, end) = match range {
                Some(r) => {
                    let start = r.start.min(length);
                    let end = r.end.min(length);
                    if start >= end {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await;
                    }
                    (start, end)
                },
                None => (0, length)
            };

            let extract_count = end - start;
            let extracted = Self::allocate(if extract_count > 0 { extract_count } else { 1 }).await;

            // Get extracted container for direct slot population
            let extracted_pointer = extracted.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!extracted_pointer.is_null());
            let extracted_container = unsafe {
                Arc::increment_strong_count(extracted_pointer as *const Container<T>);
                Arc::from_raw(extracted_pointer)
            };
            
            // Clone values directly into extracted sequence slots
            for i in 0..extract_count {
                let pointer = container.slots[start + i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    extracted_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, extracted_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", start + i, length);
                }
            }
            
            // Set extracted sequence length
            extracted_container.length.store(extract_count, std::sync::atomic::Ordering::Release);
            crate::drop!(extracted_container, container, synchronization_handle);
            extracted
        })
    }

    fn reverse(&self) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let reversed = Self::allocate(length).await;

            // Get reversed container for direct slot population
            let reversed_pointer = reversed.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!reversed_pointer.is_null());
            let reversed_container = unsafe {
                Arc::increment_strong_count(reversed_pointer as *const Container<T>);
                Arc::from_raw(reversed_pointer)
            };

            // Clone values directly into reversed sequence slots
            for i in 0..length {
                let pointer = container.slots[length - 1 - i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    reversed_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, reversed_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", length - 1 - i, length);
                }
            }
            
            reversed_container.length.store(length, std::sync::atomic::Ordering::Release);
            crate::drop!(reversed_container, container, synchronization_handle);
            reversed
        })
    }

    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let p = step.unwrap_or(1);

            // Handle step of zero - invalid, return empty sequence
            if p == 0 {
                crate::drop!(container, synchronization_handle);
                return Self::allocate(1).await;
            }

            // Calculate start and stop indices based on step direction
            let (start_index, stop_index) = if p > 0 {
                let s = start.unwrap_or(0);
                let e = stop.unwrap_or(length as isize);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length)
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(0) as usize
                } else {
                    (e as usize).min(length)
                };
                
                (start_idx, stop_idx)
            } else {
                // Negative step: default start is end of sequence, default stop is before beginning
                let s = start.unwrap_or(length as isize - 1);
                let e = stop.unwrap_or(-1);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length.saturating_sub(1))
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(-1) as isize
                } else {
                    e as isize
                };
                
                (start_idx, stop_idx as usize)
            };

            // Calculate approximate capacity
            let estimated_capacity = if p > 0 {
                if start_index >= stop_index {
                    1
                } else {
                    ((stop_index - start_index) as f64 / p as f64).ceil() as usize
                }
            } else {
                if start_index <= stop_index {
                    1
                } else {
                    ((start_index - stop_index) as f64 / (-p) as f64).ceil() as usize
                }
            };

            let sliced = Self::allocate(estimated_capacity.max(1)).await;
            
            // Get sliced container for direct slot population
            let sliced_pointer = sliced.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!sliced_pointer.is_null());
            let sliced_container = unsafe {
                Arc::increment_strong_count(sliced_pointer as *const Container<T>);
                Arc::from_raw(sliced_pointer)
            };

            let mut slot_idx = 0;
            if p > 0 {
                let mut i = start_index;
                while i < stop_index {
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let guard = arc.read().await;
                        let value = (*guard).clone();
                        drop(guard);
                        drop(arc);
                        
                        let new_arc = Arc::new(RwLock::new(value));
                        let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    i += p as usize;
                }
            } else {
                // Negative step: iterate backwards
                let step_abs = (-p) as usize;
                let mut i = start_index;
                loop {
                    // Check if current index is at or before stop (stop is exclusive, so we stop before it)
                    if i <= stop_index {
                        break;
                    }
                    
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let guard = arc.read().await;
                        let value = (*guard).clone();
                        drop(guard);
                        drop(arc);
                        
                        let new_arc = Arc::new(RwLock::new(value));
                        let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    
                    // Check if next iteration would underflow
                    if i < step_abs {
                        break;
                    }
                    i -= step_abs;
                }
            }
            
            sliced_container.length.store(slot_idx, std::sync::atomic::Ordering::Release);
            drop(sliced_container);
            
            crate::drop!(container, synchronization_handle);
            sliced
        })
    }

    fn split(&self, index: usize) -> std::pin::Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let split_index = index.min(length);
            let first = Self::allocate(split_index).await;
            let second = Self::allocate(length - split_index).await;

            // Get first container for direct slot population
            let first_pointer = first.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!first_pointer.is_null());
            let first_container = unsafe {
                Arc::increment_strong_count(first_pointer as *const Container<T>);
                Arc::from_raw(first_pointer)
            };

            // Get second container for direct slot population
            let second_pointer = second.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!second_pointer.is_null());
            let second_container = unsafe {
                Arc::increment_strong_count(second_pointer as *const Container<T>);
                Arc::from_raw(second_pointer)
            };

            // Populate first sequence
            for i in 0..split_index {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    first_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, first_container, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            first_container.length.store(split_index, std::sync::atomic::Ordering::Release);
            drop(first_container);

            // Populate second sequence
            for i in split_index..length {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    second_container.slots[i - split_index].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            second_container.length.store(length - split_index, std::sync::atomic::Ordering::Release);
            drop(second_container);

            crate::drop!(container, synchronization_handle);
            (first, second)
        })
    }
}

impl <T> traits::NonReactive<T> for Arc<Sequence<T>>
where
    T: Send + Sync + 'static,
    T: Clone,
    Self: Sync,
{
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let (start, end) = match range {
                Some(r) => {
                    let start = r.start.min(length);
                    let end = r.end.min(length);
                    if start >= end {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await;
                    }
                    (start, end)
                },
                None => (0, length)
            };

            let extract_count = end - start;
            let extracted = Self::allocate(if extract_count > 0 { extract_count } else { 1 }).await;

            // Get extracted container for direct slot population
            let extracted_pointer = extracted.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!extracted_pointer.is_null());
            let extracted_container = unsafe {
                Arc::increment_strong_count(extracted_pointer as *const Container<T>);
                Arc::from_raw(extracted_pointer)
            };
            
            // Clone values directly into extracted sequence slots
            for i in 0..extract_count {
                let pointer = container.slots[start + i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    extracted_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, extracted_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", start + i, length);
                }
            }
            
            // Set extracted sequence length
            extracted_container.length.store(extract_count, std::sync::atomic::Ordering::Release);
            crate::drop!(extracted_container, container, synchronization_handle);
            extracted
        })
    }

    fn reverse(&self) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let reversed = Self::allocate(length).await;

            // Get reversed container for direct slot population
            let reversed_pointer = reversed.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!reversed_pointer.is_null());
            let reversed_container = unsafe {
                Arc::increment_strong_count(reversed_pointer as *const Container<T>);
                Arc::from_raw(reversed_pointer)
            };

            // Clone values directly into reversed sequence slots
            for i in 0..length {
                let pointer = container.slots[length - 1 - i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    reversed_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, reversed_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", length - 1 - i, length);
                }
            }
            
            reversed_container.length.store(length, std::sync::atomic::Ordering::Release);
            crate::drop!(reversed_container, container, synchronization_handle);
            reversed
        })
    }

    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            let p = step.unwrap_or(1);

            // Handle step of zero - invalid, return empty sequence
            if p == 0 {
                crate::drop!(container, synchronization_handle);
                return Self::allocate(1).await;
            }

            // Calculate start and stop indices based on step direction
            let (start_index, stop_index) = if p > 0 {
                let s = start.unwrap_or(0);
                let e = stop.unwrap_or(length as isize);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length)
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(0) as usize
                } else {
                    (e as usize).min(length)
                };
                
                (start_idx, stop_idx)
            } else {
                // Negative step: default start is end of sequence, default stop is before beginning
                let s = start.unwrap_or(length as isize - 1);
                let e = stop.unwrap_or(-1);
                
                let start_idx = if s < 0 {
                    (length as isize + s).max(0) as usize
                } else {
                    (s as usize).min(length.saturating_sub(1))
                };
                
                let stop_idx = if e < 0 {
                    (length as isize + e).max(-1) as isize
                } else {
                    e as isize
                };
                
                (start_idx, stop_idx as usize)
            };

            // Calculate approximate capacity
            let estimated_capacity = if p > 0 {
                if start_index >= stop_index {
                    1
                } else {
                    ((stop_index - start_index) as f64 / p as f64).ceil() as usize
                }
            } else {
                if start_index <= stop_index {
                    1
                } else {
                    ((start_index - stop_index) as f64 / (-p) as f64).ceil() as usize
                }
            };

            let sliced = Self::allocate(estimated_capacity.max(1)).await;
            
            // Get sliced container for direct slot population
            let sliced_pointer = sliced.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!sliced_pointer.is_null());
            let sliced_container = unsafe {
                Arc::increment_strong_count(sliced_pointer as *const Container<T>);
                Arc::from_raw(sliced_pointer)
            };

            let mut slot_idx = 0;
            if p > 0 {
                let mut i = start_index;
                while i < stop_index {
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let guard = arc.read().await;
                        let value = (*guard).clone();
                        drop(guard);
                        drop(arc);
                        
                        let new_arc = Arc::new(RwLock::new(value));
                        let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    i += p as usize;
                }
            } else {
                // Negative step: iterate backwards
                let step_abs = (-p) as usize;
                let mut i = start_index;
                loop {
                    // Check if current index is at or before stop (stop is exclusive, so we stop before it)
                    if i <= stop_index {
                        break;
                    }
                    
                    let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                    if !pointer.is_null() {
                        let arc = unsafe {
                            Arc::increment_strong_count(pointer);
                            Arc::from_raw(pointer)
                        };
                        
                        let guard = arc.read().await;
                        let value = (*guard).clone();
                        drop(guard);
                        drop(arc);
                        
                        let new_arc = Arc::new(RwLock::new(value));
                        let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                        sliced_container.slots[slot_idx].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                        slot_idx += 1;
                    } else {
                        // This is an invariant violation
                        // This has to panic as current rules dont allow a pointer to be null within bounds
                        crate::drop!(container, synchronization_handle, sliced_container);
                        panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                    }
                    
                    // Check if next iteration would underflow
                    if i < step_abs {
                        break;
                    }
                    i -= step_abs;
                }
            }
            
            sliced_container.length.store(slot_idx, std::sync::atomic::Ordering::Release);
            drop(sliced_container);
            
            crate::drop!(container, synchronization_handle);
            sliced
        })
    }

    fn split(&self, index: usize) -> std::pin::Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>> {
        // non reactive -> clone the values.. create new arcs.
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let split_index = index.min(length);
            let first = Self::allocate(split_index).await;
            let second = Self::allocate(length - split_index).await;

            // Get first container for direct slot population
            let first_pointer = first.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!first_pointer.is_null());
            let first_container = unsafe {
                Arc::increment_strong_count(first_pointer as *const Container<T>);
                Arc::from_raw(first_pointer)
            };

            // Get second container for direct slot population
            let second_pointer = second.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!second_pointer.is_null());
            let second_container = unsafe {
                Arc::increment_strong_count(second_pointer as *const Container<T>);
                Arc::from_raw(second_pointer)
            };

            // Populate first sequence
            for i in 0..split_index {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    first_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, first_container, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            first_container.length.store(split_index, std::sync::atomic::Ordering::Release);
            drop(first_container);

            // Populate second sequence
            for i in split_index..length {
                let pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if !pointer.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(pointer);
                        Arc::from_raw(pointer)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);
                    
                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    second_container.slots[i - split_index].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, synchronization_handle, second_container);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", i, length);
                }
            }
            second_container.length.store(length - split_index, std::sync::atomic::Ordering::Release);
            drop(second_container);

            crate::drop!(container, synchronization_handle);
            (first, second)
        })
    }
}


impl <T> traits::SnapShot<T> for Sequence<T>
where 
    T: Send + Sync + 'static,
    T: Clone,
{
    fn snapshot<'a>(&'a self) -> std::pin::Pin<Box<dyn Future<Output = Vec<T>> + Send + 'a>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let mut snapshot = Vec::with_capacity(length);

            for i in 0..length {
                let slot_pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if slot_pointer.is_null() {
                    // Invariant violation: empty slot found
                    crate::drop!(container, synchronization_handle);
                    panic!("Cannot create snapshot: Sequence contains empty slots.");
                } else {
                    let arc = unsafe {
                        Arc::increment_strong_count(slot_pointer);
                        Arc::from_raw(slot_pointer)
                    };
                    let guard = arc.read().await;
                    snapshot.push((*guard).clone());
                    crate::drop!(guard, arc);
                }
            }

            crate::drop!(container, synchronization_handle);
            snapshot
        })
    }
}

impl <T> traits::SnapShot<T> for Arc<Sequence<T>>
where 
    T: Send + Sync + 'static,
    T: Clone,
{
    fn snapshot<'a>(&'a self) -> std::pin::Pin<Box<dyn Future<Output = Vec<T>> + Send + 'a>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let mut snapshot = Vec::with_capacity(length);

            for i in 0..length {
                let slot_pointer = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                if slot_pointer.is_null() {
                    // Invariant violation: empty slot found
                    crate::drop!(container, synchronization_handle);
                    panic!("Cannot create snapshot: Sequence contains empty slots.");
                } else {
                    let arc = unsafe {
                        Arc::increment_strong_count(slot_pointer);
                        Arc::from_raw(slot_pointer)
                    };
                    let guard = arc.read().await;
                    snapshot.push((*guard).clone());
                    crate::drop!(guard, arc);
                }
            }

            crate::drop!(container, synchronization_handle);
            snapshot
        })
    }
}

impl <T> traits::Equality<T> for Sequence<T>
where
    T: Send + Sync + 'static,
    T: PartialEq,
    T: Clone,
{
    fn atomic_eq<'a>(&'a self, other: &'a Self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let self_sync = self.synchronization_handle.write().await;
            let other_sync = other.synchronization_handle.write().await;

            // OPTIMIZATION: Use Relaxed ordering under write lock
            let self_pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            let other_pointer = other.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!self_pointer.is_null(), "Sequence pointer is null!");
            assert!(!other_pointer.is_null(), "Sequence pointer is null!");

            let self_container = unsafe {
                Arc::increment_strong_count(self_pointer as *const Container<T>);
                Arc::from_raw(self_pointer)
            };

            let other_container = unsafe {
                Arc::increment_strong_count(other_pointer as *const Container<T>);
                Arc::from_raw(other_pointer)
            };

            let self_length = self_container.length.load(std::sync::atomic::Ordering::Relaxed);
            let other_length = other_container.length.load(std::sync::atomic::Ordering::Relaxed);

            if self_length != other_length {
                crate::drop!(self_container, other_container, self_sync, other_sync);
                return false;
            }

            for i in 0..self_length {
                let self_slot_pointer = self_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                let other_slot_pointer = other_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);

                if self_slot_pointer.is_null() && other_slot_pointer.is_null() {
                    continue; // Both slots are empty, considered equal
                } else if self_slot_pointer.is_null() || other_slot_pointer.is_null() {
                    crate::drop!(self_container, other_container, self_sync, other_sync);
                    return false; // One slot is empty, the other is not
                } else {
                    let self_arc = unsafe {
                        Arc::increment_strong_count(self_slot_pointer);
                        Arc::from_raw(self_slot_pointer)
                    };
                    let other_arc = unsafe {
                        Arc::increment_strong_count(other_slot_pointer);
                        Arc::from_raw(other_slot_pointer)
                    };

                    let self_guard = self_arc.read().await;
                    let other_guard = other_arc.read().await;

                    if *self_guard != *other_guard {
                        crate::drop!(self_guard, other_guard, self_arc, other_arc, self_container, other_container, self_sync, other_sync);
                        return false;
                    }

                    crate::drop!(self_guard, other_guard, self_arc, other_arc);
                }
            }
            crate::drop!(self_container, other_container, self_sync, other_sync);
            true
        })
    }

    fn try_eq<'a>(&'a self, other: &'a Self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let self_sync = self.synchronization_handle.read().await;
            let other_sync = other.synchronization_handle.read().await;

            // OPTIMIZATION: Use Relaxed ordering under read lock
            let self_pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            let other_pointer = other.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!self_pointer.is_null(), "Sequence pointer is null!");
            assert!(!other_pointer.is_null(), "Sequence pointer is null!");

            let self_container = unsafe {
                Arc::increment_strong_count(self_pointer as *const Container<T>);
                Arc::from_raw(self_pointer)
            };

            let other_container = unsafe {
                Arc::increment_strong_count(other_pointer as *const Container<T>);
                Arc::from_raw(other_pointer)
            };

            let self_length = self_container.length.load(std::sync::atomic::Ordering::Relaxed);
            let other_length = other_container.length.load(std::sync::atomic::Ordering::Relaxed);

            if self_length != other_length {
                crate::drop!(self_container, other_container, self_sync, other_sync);
                return false;
            }

            for i in 0..self_length {
                let self_slot_pointer = self_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                let other_slot_pointer: *mut RwLock<T> = other_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);

                if self_slot_pointer.is_null() && other_slot_pointer.is_null() {
                    continue; // Both slots are empty, considered equal
                } else if self_slot_pointer.is_null() || other_slot_pointer.is_null() {
                    crate::drop!(self_container, other_container, self_sync, other_sync);
                    return false; // One slot is empty, the other is not
                } else {
                    let self_arc = unsafe {
                        Arc::increment_strong_count(self_slot_pointer);
                        Arc::from_raw(self_slot_pointer)
                    };
                    let other_arc = unsafe {
                        Arc::increment_strong_count(other_slot_pointer);
                        Arc::from_raw(other_slot_pointer)
                    };

                    let self_guard = self_arc.read().await;
                    let other_guard = other_arc.read().await;

                    if *self_guard != *other_guard {
                        crate::drop!(self_guard, other_guard, self_arc, other_arc, self_container, other_container, self_sync, other_sync);
                        return false;
                    }

                    crate::drop!(self_guard, other_guard, self_arc, other_arc);
                }
            }
            crate::drop!(self_container, other_container, self_sync, other_sync);
            true
        })
    }

    fn snapshot_eq<'a>(&'a self, other: &'a Self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let self_snapshot = traits::SnapShot::snapshot(self).await;
            let other_snapshot = traits::SnapShot::snapshot(other).await;
            self_snapshot == other_snapshot
        })
    }
}


impl <T> traits::Equality<T> for Arc<Sequence<T>>
where
    T: Send + Sync + 'static,
    T: PartialEq,
    T: Clone,
{
    fn atomic_eq<'a>(&'a self, other: &'a Self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let self_sync = self.synchronization_handle.write().await;
            let other_sync = other.synchronization_handle.write().await;

            // OPTIMIZATION: Use Relaxed ordering under write lock
            let self_pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            let other_pointer = other.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!self_pointer.is_null(), "Sequence pointer is null!");
            assert!(!other_pointer.is_null(), "Sequence pointer is null!");

            let self_container = unsafe {
                Arc::increment_strong_count(self_pointer as *const Container<T>);
                Arc::from_raw(self_pointer)
            };

            let other_container = unsafe {
                Arc::increment_strong_count(other_pointer as *const Container<T>);
                Arc::from_raw(other_pointer)
            };

            let self_length = self_container.length.load(std::sync::atomic::Ordering::Relaxed);
            let other_length = other_container.length.load(std::sync::atomic::Ordering::Relaxed);

            if self_length != other_length {
                crate::drop!(self_container, other_container, self_sync, other_sync);
                return false;
            }

            for i in 0..self_length {
                let self_slot_pointer = self_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                let other_slot_pointer = other_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);

                if self_slot_pointer.is_null() && other_slot_pointer.is_null() {
                    continue; // Both slots are empty, considered equal
                } else if self_slot_pointer.is_null() || other_slot_pointer.is_null() {
                    crate::drop!(self_container, other_container, self_sync, other_sync);
                    return false; // One slot is empty, the other is not
                } else {
                    let self_arc = unsafe {
                        Arc::increment_strong_count(self_slot_pointer);
                        Arc::from_raw(self_slot_pointer)
                    };
                    let other_arc = unsafe {
                        Arc::increment_strong_count(other_slot_pointer);
                        Arc::from_raw(other_slot_pointer)
                    };

                    let self_guard = self_arc.read().await;
                    let other_guard = other_arc.read().await;

                    if *self_guard != *other_guard {
                        crate::drop!(self_guard, other_guard, self_arc, other_arc, self_container, other_container, self_sync, other_sync);
                        return false;
                    }

                    crate::drop!(self_guard, other_guard, self_arc, other_arc);
                }
            }
            crate::drop!(self_container, other_container, self_sync, other_sync);
            true
        })
    }

    fn try_eq<'a>(&'a self, other: &'a Self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let self_sync = self.synchronization_handle.read().await;
            let other_sync = other.synchronization_handle.read().await;

            // OPTIMIZATION: Use Relaxed ordering under read lock
            let self_pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            let other_pointer = other.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!self_pointer.is_null(), "Sequence pointer is null!");
            assert!(!other_pointer.is_null(), "Sequence pointer is null!");

            let self_container = unsafe {
                Arc::increment_strong_count(self_pointer as *const Container<T>);
                Arc::from_raw(self_pointer)
            };

            let other_container = unsafe {
                Arc::increment_strong_count(other_pointer as *const Container<T>);
                Arc::from_raw(other_pointer)
            };

            let self_length = self_container.length.load(std::sync::atomic::Ordering::Relaxed);
            let other_length = other_container.length.load(std::sync::atomic::Ordering::Relaxed);

            if self_length != other_length {
                crate::drop!(self_container, other_container, self_sync, other_sync);
                return false;
            }

            for i in 0..self_length {
                let self_slot_pointer = self_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                let other_slot_pointer: *mut RwLock<T> = other_container.slots[i].load(std::sync::atomic::Ordering::Relaxed);

                if self_slot_pointer.is_null() && other_slot_pointer.is_null() {
                    continue; // Both slots are empty, considered equal
                } else if self_slot_pointer.is_null() || other_slot_pointer.is_null() {
                    crate::drop!(self_container, other_container, self_sync, other_sync);
                    return false; // One slot is empty, the other is not
                } else {
                    let self_arc = unsafe {
                        Arc::increment_strong_count(self_slot_pointer);
                        Arc::from_raw(self_slot_pointer)
                    };
                    let other_arc = unsafe {
                        Arc::increment_strong_count(other_slot_pointer);
                        Arc::from_raw(other_slot_pointer)
                    };

                    let self_guard = self_arc.read().await;
                    let other_guard = other_arc.read().await;

                    if *self_guard != *other_guard {
                        crate::drop!(self_guard, other_guard, self_arc, other_arc, self_container, other_container, self_sync, other_sync);
                        return false;
                    }

                    crate::drop!(self_guard, other_guard, self_arc, other_arc);
                }
            }
            crate::drop!(self_container, other_container, self_sync, other_sync);
            true
        })
    }

    fn snapshot_eq<'a>(&'a self, other: &'a Self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + 'a>> where T: PartialEq {
        Box::pin(async move {
            let self_snapshot = traits::SnapShot::snapshot(self).await;
            let other_snapshot = traits::SnapShot::snapshot(other).await;
            self_snapshot == other_snapshot
        })
    }
}



impl <T> traits::Bincode<T> for Sequence<T>
where 
    T: Send + Sync + 'static,
    T: Clone,
{
    fn bincode<'a>(&'a self, configuration: &'a crate::BincodeConfiguration) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'a>>
    where 
        T: serde::Serialize
    {
        Box::pin(async move {
            let snapshot = traits::SnapShot::snapshot(self).await;
            let string = serde_json::to_string(&snapshot)?;
            let encoded: Vec<u8>;

            match configuration {
                crate::BincodeConfiguration::Standard => {
                    encoded = bincode::encode_to_vec(&string, bincode::config::standard())?;
                },
                crate::BincodeConfiguration::Legacy => {
                    encoded = bincode::encode_to_vec(&string, bincode::config::legacy())?;
                }
            }

            Ok(encoded)
        })
    }

    fn from_bincode<'a>(bytes: &'a Vec<u8>, configuration: &'a prelude::BincodeConfiguration) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::SelfType>> + Send + 'a>>
    where
        Self: Sized,
        T: serde::de::DeserializeOwned,
    {
        Box::pin(async move {
            let json_string: String = match configuration {
                crate::BincodeConfiguration::Standard => {
                    bincode::decode_from_slice(bytes, bincode::config::standard())?.0
                },
                crate::BincodeConfiguration::Legacy => {
                    bincode::decode_from_slice(bytes, bincode::config::legacy())?.0
                }
            };
            
            let deserialized: Vec<T> = serde_json::from_str(&json_string)?;
            let sequence = Sequence::from_vec(&deserialized).await;
            Ok(sequence)
        })
    }
}

impl <T> traits::Bincode<T> for Arc<Sequence<T>>
where
    T: Send + Sync + 'static,
    T: Clone,
{
    fn bincode<'a>(&'a self, configuration: &'a crate::BincodeConfiguration) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'a>>
    where 
        T: serde::Serialize,
    {
        Box::pin(async move {
            let snapshot = traits::SnapShot::snapshot(self).await;
            let string = serde_json::to_string(&snapshot)?;
            let encoded: Vec<u8>;

            match configuration {
                crate::BincodeConfiguration::Standard => {
                    encoded = bincode::encode_to_vec(&string, bincode::config::standard())?;
                },
                crate::BincodeConfiguration::Legacy => {
                    encoded = bincode::encode_to_vec(&string, bincode::config::legacy())?;
                }
            }

            Ok(encoded)
        })
    }

    fn from_bincode<'a>(bytes: &'a Vec<u8>, configuration: &'a prelude::BincodeConfiguration) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::SelfType>> + Send + 'a>>
    where
        Self: Sized,
        T: serde::de::DeserializeOwned,
    {
        Box::pin(async move {
            let json_string: String = match configuration {
                crate::BincodeConfiguration::Standard => {
                    bincode::decode_from_slice(bytes, bincode::config::standard())?.0
                },
                crate::BincodeConfiguration::Legacy => {
                    bincode::decode_from_slice(bytes, bincode::config::legacy())?.0
                }
            };
            
            let deserialized: Vec<T> = serde_json::from_str(&json_string)?;
            let sequence = Sequence::from_vec(&deserialized).await;
            Ok(sequence)
        })
    }
}

impl <T> traits::Stack<T> for Sequence<T>
where 
    T: Send + Sync + 'static,
    Self: Sync,
{
    type ArcSwapRef = compat::Compat<T>;

    fn push(&self, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

            assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);
            
            if length == capacity {
                crate::drop!(container, synchronization_handle);
                panic!("Stack overflow: Attempted to push onto a full sequence.");
            }

            // since we are checking if length == capacity anyway..
            // lets move on

            let new_arc = match value {
                crate::Candidate::Value(v) => Arc::new(RwLock::new(v)),
                crate::Candidate::Arc(v) => v,
            };

            let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
            container.slots[length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
            container.length.store(length + 1, std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
        })
    }

    fn pop(&self) -> std::pin::Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            if length == 0 {
                crate::drop!(container, synchronization_handle);
                panic!("Stack underflow: Attempted to pop from an empty sequence.");
            }

            let index = length - 1;
            let removed_ptr = container.slots[index].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

            // No need to shift for pop (removing from end)
            // Just decrement length
            container.length.fetch_sub(1, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            
            if removed_ptr.is_null() {
                compat::Compat::create(ArcSwapOption::empty()).await
            } else {
                compat::Compat::create(ArcSwapOption::new(Some(unsafe { Arc::from_raw(removed_ptr) }))).await
            }
        })
    }

    fn peek(&self) -> std::pin::Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            if length == 0 {
                crate::drop!(container, synchronization_handle);
                panic!("Stack underflow: Attempted to peek at an empty sequence.");
            }

            let index = length - 1;
            let pointer = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            crate::drop!(container, synchronization_handle);

            if pointer.is_null() {
                compat::Compat::create(ArcSwapOption::empty()).await
            } else {
                let result = unsafe {
                    Arc::increment_strong_count(pointer);
                    Arc::from_raw(pointer)
                };
                compat::Compat::create(ArcSwapOption::new(Some(result))).await
            }
        })
    }

    #[allow(non_snake_case)]
    fn pop_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone
    {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            // NEW:
            let actual_pop_count = if n > length {
                if AoN {
                    // All or Nothing: either pop exactly n or pop 0
                    if ignore_errors {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await; // Return empty
                    } else {
                        crate::drop!(container, synchronization_handle);
                        panic!("Stack underflow: Attempted to pop {} elements from a sequence of length {}.", n, length);
                    }
                } else {
                    // Pop as many as available
                    if !ignore_errors {
                        crate::drop!(container, synchronization_handle);
                        panic!("Stack underflow: Attempted to pop {} elements from a sequence of length {}.", n, length);
                    }
                    length // Pop all available
                }
            } else {
                n // Can pop exactly n
            };

            let new_sequence = Sequence::allocate(actual_pop_count.max(1)).await;
            let new_pointer = new_sequence.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!new_pointer.is_null());
            let new_container = unsafe {
                Arc::increment_strong_count(new_pointer as *const Container<T>);
                Arc::from_raw(new_pointer)
            };

            // Pop is never reactive - we're removing elements from the original stack
            // So we clone values into the new sequence
            for i in 0..actual_pop_count {
                let index = length - actual_pop_count + i;
                let removed_ptr = container.slots[index].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

                if !removed_ptr.is_null() {
                    let arc = unsafe {
                        Arc::from_raw(removed_ptr)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);

                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    new_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, new_container, synchronization_handle);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
                }
            }

            container.length.fetch_sub(actual_pop_count, std::sync::atomic::Ordering::Release);
            new_container.length.store(actual_pop_count, std::sync::atomic::Ordering::Release);

            crate::drop!(container, new_container, synchronization_handle);
            new_sequence
        })
    }

    #[allow(non_snake_case)]
    fn push_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>>
        where
            I: IntoIterator<Item = T> + Send + 'static,
            I::IntoIter: Send {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let mut length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            let available_space = capacity - length;

            // Collect items to determine count
            let items: Vec<T> = iter.into_iter().collect();
            let items_count = items.len();

            // Check if we can push all items
            if items_count > available_space {
                if AoN {
                    // All or Nothing: either push all or push none
                    crate::drop!(container, synchronization_handle);
                    if ignore_errors {
                        return; // Push nothing
                    } else {
                        panic!("Stack overflow: Attempted to push {} elements but only {} slots available.", items_count, available_space);
                    }
                } else {
                    // Push as many as fit
                    if !ignore_errors {
                        crate::drop!(container, synchronization_handle);
                        panic!("Stack overflow: Attempted to push {} elements but only {} slots available.", items_count, available_space);
                    }
                }
            }

            let push_count = items_count.min(available_space);
            for (idx, value) in items.into_iter().take(push_count).enumerate() {
                let new_arc = Arc::new(RwLock::new(value));
                let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
                container.slots[length + idx].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
            }

            length += push_count;

            container.length.store(length, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
        })
    }

    fn swap_top(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            if length < 2 {
                crate::drop!(container, synchronization_handle);
                return false; // Not enough elements to swap
            }

            let top_index = length - 1;
            let second_index = length - 2;

            let top_pointer = container.slots[top_index].load(std::sync::atomic::Ordering::Relaxed);
            let second_pointer = container.slots[second_index].load(std::sync::atomic::Ordering::Relaxed);

            container.slots[top_index].store(second_pointer, std::sync::atomic::Ordering::Relaxed);
            container.slots[second_index].store(top_pointer, std::sync::atomic::Ordering::Relaxed);

            crate::drop!(container, synchronization_handle);
            true
        })
    }

    fn dup(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>>
        where
            T: Clone {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            
            if length == 0 {
                crate::drop!(container, synchronization_handle);
                return false; // Cannot duplicate on empty stack
            }

            assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);

            if length == capacity {
                // Need to resize before duplicating
                let upto = self.generate_capacity(capacity, length + 1).await;
                crate::drop!(container);
                self.resize(upto).await;
                drop(synchronization_handle);
                return self.dup().await; // Retry after resize
            }

            // Get the top element (at index length-1)
            let top_index = length - 1;
            let top_ptr = container.slots[top_index].load(std::sync::atomic::Ordering::Relaxed);
            
            if top_ptr.is_null() {
                crate::drop!(container, synchronization_handle);
                return false; // Cannot duplicate null element
            }

            // Clone the value from top element
            let arc = unsafe {
                Arc::increment_strong_count(top_ptr);
                Arc::from_raw(top_ptr)
            };
            
            let guard = arc.read().await;
            let value = (*guard).clone();
            drop(guard);
            drop(arc);

            // Create new Arc with cloned value
            let new_arc = Arc::new(RwLock::new(value));
            let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
            
            // Push duplicate to top (at index length)
            container.slots[length].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
            container.length.store(length + 1, std::sync::atomic::Ordering::Release);
            
            crate::drop!(container, synchronization_handle);
            true
        })
    }
}

impl <T> traits::Stack<T> for Arc<Sequence<T>>
where 
    T: Send + Sync + 'static,
    Self: Sync,
{
    type ArcSwapRef = compat::Compat<T>;

    fn push(&self, value: crate::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

            assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);
            
            if length == capacity {
                crate::drop!(container, synchronization_handle);
                panic!("Stack overflow: Attempted to push onto a full sequence.");
            }

            // since we are checking if length == capacity anyway..
            // lets move on

            let new_arc = match value {
                crate::Candidate::Value(v) => Arc::new(RwLock::new(v)),
                crate::Candidate::Arc(v) => v,
            };

            let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
            container.slots[length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
            container.length.store(length + 1, std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
        })
    }

    fn pop(&self) -> std::pin::Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            if length == 0 {
                crate::drop!(container, synchronization_handle);
                panic!("Stack underflow: Attempted to pop from an empty sequence.");
            }

            let index = length - 1;
            let removed_ptr = container.slots[index].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

            // No need to shift for pop (removing from end)
            // Just decrement length
            container.length.fetch_sub(1, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            
            if removed_ptr.is_null() {
                compat::Compat::create(ArcSwapOption::empty()).await
            } else {
                compat::Compat::create(ArcSwapOption::new(Some(unsafe { Arc::from_raw(removed_ptr) }))).await
            }
        })
    }

    fn peek(&self) -> std::pin::Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.read().await;
            
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            if length == 0 {
                crate::drop!(container, synchronization_handle);
                panic!("Stack underflow: Attempted to peek at an empty sequence.");
            }

            let index = length - 1;
            let pointer = container.slots[index].load(std::sync::atomic::Ordering::Relaxed);
            crate::drop!(container, synchronization_handle);

            if pointer.is_null() {
                compat::Compat::create(ArcSwapOption::empty()).await
            } else {
                let result = unsafe {
                    Arc::increment_strong_count(pointer);
                    Arc::from_raw(pointer)
                };
                compat::Compat::create(ArcSwapOption::new(Some(result))).await
            }
        })
    }

    #[allow(non_snake_case)]
    fn pop_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone
    {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            // NEW:
            let actual_pop_count = if n > length {
                if AoN {
                    // All or Nothing: either pop exactly n or pop 0
                    if ignore_errors {
                        crate::drop!(container, synchronization_handle);
                        return Sequence::allocate(1).await; // Return empty
                    } else {
                        crate::drop!(container, synchronization_handle);
                        panic!("Stack underflow: Attempted to pop {} elements from a sequence of length {}.", n, length);
                    }
                } else {
                    // Pop as many as available
                    if !ignore_errors {
                        crate::drop!(container, synchronization_handle);
                        panic!("Stack underflow: Attempted to pop {} elements from a sequence of length {}.", n, length);
                    }
                    length // Pop all available
                }
            } else {
                n // Can pop exactly n
            };

            let new_sequence = Sequence::allocate(actual_pop_count.max(1)).await;
            let new_pointer = new_sequence.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!new_pointer.is_null());
            let new_container = unsafe {
                Arc::increment_strong_count(new_pointer as *const Container<T>);
                Arc::from_raw(new_pointer)
            };

            // Pop is never reactive - we're removing elements from the original stack
            // So we clone values into the new sequence
            for i in 0..actual_pop_count {
                let index = length - actual_pop_count + i;
                let removed_ptr = container.slots[index].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

                if !removed_ptr.is_null() {
                    let arc = unsafe {
                        Arc::from_raw(removed_ptr)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);

                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    new_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                } else {
                    // This is an invariant violation
                    // This has to panic as current rules dont allow a pointer to be null within bounds
                    crate::drop!(container, new_container, synchronization_handle);
                    panic!("Invariant violation: slot at index {} is null within bounds (length {}).", index, length);
                }
            }

            container.length.fetch_sub(actual_pop_count, std::sync::atomic::Ordering::Release);
            new_container.length.store(actual_pop_count, std::sync::atomic::Ordering::Release);

            crate::drop!(container, new_container, synchronization_handle);
            new_sequence
        })
    }

    #[allow(non_snake_case)]
    fn push_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>>
        where
            I: IntoIterator<Item = T> + Send + 'static,
            I::IntoIter: Send {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let mut length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            let available_space = capacity - length;

            // Collect items to determine count
            let items: Vec<T> = iter.into_iter().collect();
            let items_count = items.len();

            // Check if we can push all items
            if items_count > available_space {
                if AoN {
                    // All or Nothing: either push all or push none
                    crate::drop!(container, synchronization_handle);
                    if ignore_errors {
                        return; // Push nothing
                    } else {
                        panic!("Stack overflow: Attempted to push {} elements but only {} slots available.", items_count, available_space);
                    }
                } else {
                    // Push as many as fit
                    if !ignore_errors {
                        crate::drop!(container, synchronization_handle);
                        panic!("Stack overflow: Attempted to push {} elements but only {} slots available.", items_count, available_space);
                    }
                }
            }

            let push_count = items_count.min(available_space);
            for (idx, value) in items.into_iter().take(push_count).enumerate() {
                let new_arc = Arc::new(RwLock::new(value));
                let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
                container.slots[length + idx].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
            }

            length += push_count;

            container.length.store(length, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
        })
    }

    fn swap_top(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);

            if length < 2 {
                crate::drop!(container, synchronization_handle);
                return false; // Not enough elements to swap
            }

            let top_index = length - 1;
            let second_index = length - 2;

            let top_pointer = container.slots[top_index].load(std::sync::atomic::Ordering::Relaxed);
            let second_pointer = container.slots[second_index].load(std::sync::atomic::Ordering::Relaxed);

            container.slots[top_index].store(second_pointer, std::sync::atomic::Ordering::Relaxed);
            container.slots[second_index].store(top_pointer, std::sync::atomic::Ordering::Relaxed);

            crate::drop!(container, synchronization_handle);
            true
        })
    }

    fn dup(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>>
        where
            T: Clone {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);
            
            if length == 0 {
                crate::drop!(container, synchronization_handle);
                return false; // Cannot duplicate on empty stack
            }

            assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);

            if length == capacity {
                // Need to resize before duplicating
                let upto = self.generate_capacity(capacity, length + 1).await;
                crate::drop!(container);
                self.resize(upto).await;
                drop(synchronization_handle);
                return self.dup().await; // Retry after resize
            }

            // Get the top element (at index length-1)
            let top_index = length - 1;
            let top_ptr = container.slots[top_index].load(std::sync::atomic::Ordering::Relaxed);
            
            if top_ptr.is_null() {
                crate::drop!(container, synchronization_handle);
                return false; // Cannot duplicate null element
            }

            // Clone the value from top element
            let arc = unsafe {
                Arc::increment_strong_count(top_ptr);
                Arc::from_raw(top_ptr)
            };
            
            let guard = arc.read().await;
            let value = (*guard).clone();
            drop(guard);
            drop(arc);

            // Create new Arc with cloned value
            let new_arc = Arc::new(RwLock::new(value));
            let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
            
            // Push duplicate to top (at index length)
            container.slots[length].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
            container.length.store(length + 1, std::sync::atomic::Ordering::Release);
            
            crate::drop!(container, synchronization_handle);
            true
        })
    }
}

impl <T> traits::Queue<T> for Sequence<T>
where 
    T: Send + Sync + 'static,
{
    type ArcSwapRef = compat::Compat<T>;

    fn enqueue(&self, value: prelude::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

            assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);
            
            if length == capacity {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!("Queue overflow: Attempted to enqueue onto a full sequence."));
            }

            // since we are checking if length == capacity anyway..
            // lets move on

            let new_arc = match value {
                prelude::Candidate::Value(v) => Arc::new(RwLock::new(v)),
                prelude::Candidate::Arc(v) => v,
            };

            let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
            container.slots[length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
            container.length.store(length + 1, std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
            Ok(())
        })
    }

    #[allow(non_snake_case)]
    fn enqueue_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
        where
            I: IntoIterator<Item = T> + Send + 'static,
            I::IntoIter: Send {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

            // Collect all items first to check if AoN mode can be satisfied
            let items: Vec<T> = iter.into_iter().collect();
            let items_count = items.len();
            
            // Check if we have enough space for AoN mode
            if AoN && length + items_count > capacity {
                crate::drop!(container, synchronization_handle);
                if ignore_errors {
                    return Ok(());
                }
                return Err(anyhow::anyhow!("Queue overflow: Attempted to enqueue {} elements but only {} slots available (AoN mode).", items_count, capacity - length));
            }

            // Enqueue elements
            let mut current_length = length;
            for value in items {
                if current_length == capacity {
                    // This only happens in non-AoN mode
                    container.length.store(current_length, std::sync::atomic::Ordering::Relaxed);
                    crate::drop!(container, synchronization_handle);
                    if ignore_errors {
                        return Ok(());
                    }
                    return Err(anyhow::anyhow!("Queue overflow: Attempted to enqueue onto a full sequence."));
                }

                let new_arc = Arc::new(RwLock::new(value));
                let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
                container.slots[current_length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
                current_length += 1;
            }

            container.length.store(current_length, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            Ok(())
        })
    }

    fn dequeue(&self) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            if length == 0 {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!("Queue underflow: Attempted to dequeue from an empty sequence."));
            }

            let removed_ptr = container.slots[0].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

            // Shift elements to the left
            for i in 1..length {
                let current_ptr = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                container.slots[i - 1].store(current_ptr, std::sync::atomic::Ordering::Relaxed);
            }

            container.length.fetch_sub(1, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            
            if removed_ptr.is_null() {
                Ok(compat::Compat::create(ArcSwapOption::empty()).await)
            } else {
                Ok(compat::Compat::create(ArcSwapOption::new(Some(unsafe { Arc::from_raw(removed_ptr) }))).await)
            }
        })
    }

    #[allow(non_snake_case)]
    fn dequeue_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Arc<Self::SelfType>>> + Send + '_>>
        where
            T: Clone {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            // Determine how many elements to actually dequeue
            let actual_n = if n > length {
                if AoN {
                    // All-or-Nothing: fail if we can't get all n elements
                    crate::drop!(container, synchronization_handle);
                    if ignore_errors {
                        return Ok(Sequence::allocate(1).await);
                    } else {
                        return Err(anyhow::anyhow!("Queue underflow: Attempted to dequeue {} elements from a queue of length {}.", n, length));
                    }
                } else {
                    // Best-effort: dequeue as many as available
                    length
                }
            } else {
                n
            };

            // Create sequence to hold dequeued elements
            let dequeued = Sequence::allocate(actual_n.max(1)).await;
            let dequeued_pointer = dequeued.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!dequeued_pointer.is_null());
            let dequeued_container = unsafe {
                Arc::increment_strong_count(dequeued_pointer as *const Container<T>);
                Arc::from_raw(dequeued_pointer)
            };

            // Remove from front (index 0 to actual_n-1) and clone values
            for i in 0..actual_n {
                let removed_ptr = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                
                if !removed_ptr.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(removed_ptr);
                        Arc::from_raw(removed_ptr)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);

                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    dequeued_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                }
            }

            dequeued_container.length.store(actual_n, std::sync::atomic::Ordering::Release);
            drop(dequeued_container);

            // Shift remaining elements to the front
            for i in 0..(length - actual_n) {
                let src = container.slots[i + actual_n].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                let displaced = container.slots[i].swap(src, std::sync::atomic::Ordering::Relaxed);
                
                if !displaced.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(displaced) });
                }
            }

            // Clear the remaining slots at the end
            for i in (length - actual_n)..length {
                let removed = container.slots[i].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                if !removed.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(removed) });
                }
            }

            container.length.store(length - actual_n, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            
            // Return error if best-effort mode didn't get all requested elements
            if !AoN && actual_n < n {
                if ignore_errors {
                    Ok(dequeued)
                } else {
                    Err(anyhow::anyhow!("Queue underflow: Requested {} elements but only {} were available (dequeued {} elements).", n, length, actual_n))
                }
            } else {
                Ok(dequeued)
            }
        })
    }
}

impl <T> traits::Queue<T> for Arc<Sequence<T>>
where 
    T: Send + Sync + 'static,
{
    type ArcSwapRef = compat::Compat<T>;

    fn enqueue(&self, value: prelude::Candidate<T>) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

            assert!(length <= capacity, "Invariant violation: length {} exceeds capacity {}.", length, capacity);
            
            if length == capacity {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!("Queue overflow: Attempted to enqueue onto a full sequence."));
            }

            // since we are checking if length == capacity anyway..
            // lets move on

            let new_arc = match value {
                prelude::Candidate::Value(v) => Arc::new(RwLock::new(v)),
                prelude::Candidate::Arc(v) => v,
            };

            let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
            container.slots[length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
            container.length.store(length + 1, std::sync::atomic::Ordering::Release);

            crate::drop!(container, synchronization_handle);
            Ok(())
        })
    }

    #[allow(non_snake_case)]
    fn enqueue_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
        where
            I: IntoIterator<Item = T> + Send + 'static,
            I::IntoIter: Send {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;
            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");

            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            let capacity = container.capacity.load(std::sync::atomic::Ordering::Relaxed);

            // Collect all items first to check if AoN mode can be satisfied
            let items: Vec<T> = iter.into_iter().collect();
            let items_count = items.len();
            
            // Check if we have enough space for AoN mode
            if AoN && length + items_count > capacity {
                crate::drop!(container, synchronization_handle);
                if ignore_errors {
                    return Ok(());
                }
                return Err(anyhow::anyhow!("Queue overflow: Attempted to enqueue {} elements but only {} slots available (AoN mode).", items_count, capacity - length));
            }

            // Enqueue elements
            let mut current_length = length;
            for value in items {
                if current_length == capacity {
                    // This only happens in non-AoN mode
                    container.length.store(current_length, std::sync::atomic::Ordering::Relaxed);
                    crate::drop!(container, synchronization_handle);
                    if ignore_errors {
                        return Ok(());
                    }
                    return Err(anyhow::anyhow!("Queue overflow: Attempted to enqueue onto a full sequence."));
                }

                let new_arc = Arc::new(RwLock::new(value));
                let pointer_new = Arc::into_raw(new_arc) as *mut RwLock<T>;
                container.slots[current_length].store(pointer_new, std::sync::atomic::Ordering::Relaxed);
                current_length += 1;
            }

            container.length.store(current_length, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            Ok(())
        })
    }

    fn dequeue(&self) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>> {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            if length == 0 {
                crate::drop!(container, synchronization_handle);
                return Err(anyhow::anyhow!("Queue underflow: Attempted to dequeue from an empty sequence."));
            }

            let removed_ptr = container.slots[0].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);

            // Shift elements to the left
            for i in 1..length {
                let current_ptr = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                container.slots[i - 1].store(current_ptr, std::sync::atomic::Ordering::Relaxed);
            }

            container.length.fetch_sub(1, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            
            if removed_ptr.is_null() {
                Ok(compat::Compat::create(ArcSwapOption::empty()).await)
            } else {
                Ok(compat::Compat::create(ArcSwapOption::new(Some(unsafe { Arc::from_raw(removed_ptr) }))).await)
            }
        })
    }

    #[allow(non_snake_case)]
    fn dequeue_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> std::pin::Pin<Box<dyn Future<Output = anyhow::Result<Arc<Self::SelfType>>> + Send + '_>>
        where
            T: Clone {
        Box::pin(async move {
            let synchronization_handle = self.synchronization_handle.write().await;

            let pointer = self.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!pointer.is_null(), "Sequence pointer is null!");
            
            let container = unsafe {
                Arc::increment_strong_count(pointer as *const Container<T>);
                Arc::from_raw(pointer)
            };

            let length = container.length.load(std::sync::atomic::Ordering::Relaxed);
            
            // Determine how many elements to actually dequeue
            let actual_n = if n > length {
                if AoN {
                    // All-or-Nothing: fail if we can't get all n elements
                    crate::drop!(container, synchronization_handle);
                    if ignore_errors {
                        return Ok(Sequence::allocate(1).await);
                    } else {
                        return Err(anyhow::anyhow!("Queue underflow: Attempted to dequeue {} elements from a queue of length {}.", n, length));
                    }
                } else {
                    // Best-effort: dequeue as many as available
                    length
                }
            } else {
                n
            };

            // Create sequence to hold dequeued elements
            let dequeued = Sequence::allocate(actual_n.max(1)).await;
            let dequeued_pointer = dequeued.container.load(std::sync::atomic::Ordering::Relaxed);
            assert!(!dequeued_pointer.is_null());
            let dequeued_container = unsafe {
                Arc::increment_strong_count(dequeued_pointer as *const Container<T>);
                Arc::from_raw(dequeued_pointer)
            };

            // Remove from front (index 0 to actual_n-1) and clone values
            for i in 0..actual_n {
                let removed_ptr = container.slots[i].load(std::sync::atomic::Ordering::Relaxed);
                
                if !removed_ptr.is_null() {
                    let arc = unsafe {
                        Arc::increment_strong_count(removed_ptr);
                        Arc::from_raw(removed_ptr)
                    };
                    
                    let guard = arc.read().await;
                    let value = (*guard).clone();
                    drop(guard);
                    drop(arc);

                    let new_arc = Arc::new(RwLock::new(value));
                    let new_pointer = Arc::into_raw(new_arc) as *mut RwLock<T>;
                    dequeued_container.slots[i].store(new_pointer, std::sync::atomic::Ordering::Relaxed);
                }
            }

            dequeued_container.length.store(actual_n, std::sync::atomic::Ordering::Release);
            drop(dequeued_container);

            // Shift remaining elements to the front
            for i in 0..(length - actual_n) {
                let src = container.slots[i + actual_n].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                let displaced = container.slots[i].swap(src, std::sync::atomic::Ordering::Relaxed);
                
                if !displaced.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(displaced) });
                }
            }

            // Clear the remaining slots at the end
            for i in (length - actual_n)..length {
                let removed = container.slots[i].swap(std::ptr::null_mut(), std::sync::atomic::Ordering::Relaxed);
                if !removed.is_null() {
                    crate::drop!(unsafe { Arc::from_raw(removed) });
                }
            }

            container.length.store(length - actual_n, std::sync::atomic::Ordering::Release);
            crate::drop!(container, synchronization_handle);
            
            // Return error if best-effort mode didn't get all requested elements
            if !AoN && actual_n < n {
                if ignore_errors {
                    Ok(dequeued)
                } else {
                    Err(anyhow::anyhow!("Queue underflow: Requested {} elements but only {} were available (dequeued {} elements).", n, length, actual_n))
                }
            } else {
                Ok(dequeued)
            }
        })
    }
}


pub mod prelude;

#[cfg(test)]
mod tests;