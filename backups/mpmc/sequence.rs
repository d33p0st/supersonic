
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::Index;
use std::path::Iter;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::cell::Cell;
use std::{ptr, vec};


// Random seed generator.
macro_rules! seed {
    () => {{
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
        nanos ^ (nanos.rotate_left(32))
    }};
}


// Thread-local RNG for random operations.
thread_local! { static RNG: Cell<u64> = Cell::new(seed!()) }

struct RandomIndex;

impl RandomIndex {
    fn new(limit: usize) -> usize {
        RNG.with(|r| {
            let mut x = r.get();
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            r.set(x);
            x.wrapping_mul(0x2545F4914F6CDD1D)
        }) as usize % limit
    }
}

/// Cache-line aligned slot to prevent false sharing
#[repr(align(64))]
struct Unit<T> {
    data: AtomicPtr<T>,
}

impl<T> Unit<T> {
    fn new() -> Self {
        Self { data: AtomicPtr::new(ptr::null_mut()) }
    }
}

/// A fixed-capacity, lock-free, concurrent sequence backed by atomic pointers.
///
/// `Sequence<T>` stores elements in a preallocated array of slots (`Unit<T>`),
/// where each slot holds an `AtomicPtr<T>`. Slots may be independently written,
/// replaced, or freed by multiple threads without locks.
/// 
/// ```
/// use supersonic::mpmc::sequence::Sequence;
/// 
/// // creation
/// let sequence = Sequence::<i32>::new(5);
/// 
/// // creation from other data types
/// // 1. from vector
/// let vector = vec![1, 2, 3];
/// let sequence = Sequence::from(vector);
/// 
/// // 2. from HashSet.
/// let hashset = std::collections::HashSet::new();
/// let sequence = Sequence::<i32>::from(hashset);
/// 
/// // 3. from HashMap
/// use supersonic::mpmc::sequence::HashMapToSequence;
/// use std::collections::HashMap;
/// 
/// let hashmap: HashMap<usize, usize> = HashMap::new();
/// let hashmap_to_sequence = Sequence::<usize>::from_hashmap(hashmap.clone(), false);
/// let hashmap_to_sequence2 = Sequence::<usize>::from_hashmap(hashmap, true);
/// let sequence_of_keys = hashmap_to_sequence.from_keys().unwrap();
/// let sequence_of_values = hashmap_to_sequence2.from_values().unwrap();
/// 
/// // Other data types include from another Sequence<T> and LinkedList<T>.
/// // NOTE: default placeholder sequences can be created which has capacity of 1.
/// // (capacity > 0, always).
/// let sequence = Sequence::<String>::default();
/// ```
///
/// This structure is optimized for:
/// - High-concurrency insertion and removal
/// - Random-access writes
/// - Snapshot-style reads
/// - Low false-sharing (cache-line aligned)
///
/// ## Design Characteristics
/// - **Fixed capacity**: Capacity is chosen at construction time and never grows.
/// - **Sparse storage**: Slots may be empty or occupied.
/// - **Lock-free**: All operations rely on atomic primitives.
/// - **Cache-line aligned**: Both the sequence and its slots are aligned to 64 bytes
///   to minimize false sharing across threads.
///
/// ## Concurrency Model
/// - All mutation operations are lock-free.
/// - Reads may race with writes.
/// - The container provides *eventual consistency*, not linearizability.
/// - No global ordering guarantees are provided between operations on different indices.
///
/// ## Memory Model
/// - Each element is heap-allocated (`Box<T>`) and referenced via `AtomicPtr<T>`.
/// - Ownership of elements is transferred atomically between threads.
/// - Elements may be dropped at any time due to concurrent removal or replacement.
///
/// ## Safety & Lifetime Guarantees
/// ⚠️ **Important**:
/// - Any reference (`&T`) obtained from this container is **ephemeral**.
/// - References are only valid at the moment they are observed.
/// - Concurrent mutation may invalidate references immediately after they are returned.
///
/// This container intentionally bypasses Rust’s borrow checker using atomics.
/// Correct usage relies on API discipline and documentation, not compiler enforcement.
///
/// ## Length Semantics
/// - `length` tracks the number of *currently occupied* slots.
/// - Under concurrency, `length()` is an approximation.
/// - Use `consistent_length()` if you need a stabilized read.
///
/// ## When to Use
/// - Concurrent caches
/// - Lock-free work queues
/// - Slot-based registries
/// - High-performance concurrent sets with relaxed ordering
///
/// ## When NOT to Use
/// - When strict iteration stability is required
/// - When references must remain valid across time
/// - When dynamic resizing is needed
///
/// ## Type Parameters
/// - `T`: Stored value type. Must be `Send` for cross-thread usage.
///
/// ## Thread Safety
/// - `Sequence<T>` is `Send` and `Sync` **iff** `T: Send`.
#[repr(align(64))]
pub struct Sequence<T> {
    /// Array of cache-line-aligned slots holding atomic pointers to elements.
    ///
    /// Each slot may independently transition between:
    /// - empty (`null`)
    /// - occupied (valid pointer)
    ///
    /// Slots do not coordinate with each other.
    units: Box<[Unit<T>]>,

    /// Maximum number of slots in the sequence.
    ///
    /// This value never changes after construction.
    capacity: usize,

    /// Approximate count of occupied slots.
    ///
    /// Updated using atomic increments/decrements.
    /// Under contention, this value may be temporarily inconsistent.
    length: AtomicUsize,
}

impl<T> From<Vec<T>> for Sequence<T> {
    fn from(value: Vec<T>) -> Self {
        let sequence = Sequence::new(value.len().max(1));
        let mut index = 0;
        for v in value {
            sequence.write(index, v);
            index += 1;
        }
        sequence
    }
}

impl<T> From<std::collections::HashSet<T>> for Sequence<T> {
    fn from(value: std::collections::HashSet<T>) -> Self {
        let sequence = Sequence::new(value.capacity().max(1));
        let mut index = 0;
        for element in value {
            sequence.write(index, element);
            index += 1;
        }
        sequence
    }
}

impl<T> From<std::collections::LinkedList<T>> for Sequence<T> {
    fn from(value: std::collections::LinkedList<T>) -> Self {
        let sequence = Sequence::new(value.len().max(1));
        let mut index= 0;
        for element in value {
            sequence.write(index, element);
            index += 1;
        }
        sequence
    }
}

impl<T> Into<Vec<T>> for Sequence<T>
where
    T: Clone,
{
    fn into(self) -> Vec<T> {
        self.to_vec()
    }
}

impl<T> Into<std::collections::HashSet<T>> for Sequence<T>
where
    T: Clone + Eq + Hash,
{
    fn into(self) -> std::collections::HashSet<T> {
        let mut hashset = std::collections::HashSet::new();
        self.for_each(|_, value| {
            hashset.insert(value.clone());
        });
        hashset
    }
}

impl<T> Into<String> for Sequence<T>
where
    T: Display
{
    fn into(self) -> String {
        self.to_string(",")
    }
}


/// Result type for converting a `HashMap` into a `Sequence`.
///
/// This enum is used to preserve type safety when choosing between
/// extracting keys or values from a `HashMap`.
pub enum HashMapToSequence<K, V> {
	/// Contains a `Sequence` constructed from the map's keys.
	FromKeys(Sequence<K>),

	/// Contains a `Sequence` constructed from the map's values.
	FromValues(Sequence<V>),
}

impl<K, V> HashMapToSequence<K, V>
where
	K: Clone,
	V: Clone,
{
	/// Returns a cloned `Sequence<K>` if this instance contains keys.
	///
	/// # Behavior
	/// - If the enum variant is `FromKeys`, returns a **new Sequence**
	///   containing clones of all keys.
	/// - If the variant is `FromValues`, returns `None`.
	///
	/// # Consistency
	/// - The returned sequence is a **snapshot** and is fully independent
	///   of the original internal `Sequence`.
	///
	/// # Performance
	/// - Time complexity: **O(n)**
	/// - Allocations:
	///   - One allocation for the new `Sequence`
	///   - One allocation per cloned element
	///
	/// # Warnings
	/// - This performs a **full clone** of all elements.
	/// - No ordering guarantees are provided.
	pub fn from_keys(&self) -> Option<Sequence<K>> {
		match self {
			Self::FromKeys(x) => {
				let v = x.to_vec();
				Some(Sequence::from(v))
			}
			Self::FromValues(_) => None,
		}
	}

	/// Returns a cloned `Sequence<V>` if this instance contains values.
	///
	/// # Behavior
	/// - If the enum variant is `FromValues`, returns a **new Sequence**
	///   containing clones of all values.
	/// - If the variant is `FromKeys`, returns `None`.
	///
	/// # Consistency
	/// - The returned sequence is a **snapshot** and is fully independent
	///   of the original internal `Sequence`.
	///
	/// # Performance
	/// - Time complexity: **O(n)**
	/// - Allocations:
	///   - One allocation for the new `Sequence`
	///   - One allocation per cloned element
	///
	/// # Warnings
	/// - This performs a **full clone** of all elements.
	/// - No ordering guarantees are provided.
	pub fn from_values(&self) -> Option<Sequence<V>> {
		match self {
			Self::FromKeys(_) => None,
			Self::FromValues(x) => {
				let v = x.to_vec();
				Some(Sequence::from(v))
			}
		}
	}
}


pub struct SequenceStats {
    pub capacity: usize,
    pub length: usize,
    pub occupancy: f32,
}



impl<T> Sequence<T> {

    /// Creates a new empty `Sequence` with a fixed capacity.
    ///
    /// All slots are preallocated and initialized as empty. The capacity is
    /// fixed for the lifetime of the sequence and cannot be resized.
    ///
    /// ## Behavior
    /// - Allocates `capacity` independent slots.
    /// - Each slot starts in the empty state (`null` pointer).
    /// - Initializes the length counter to `0`.
    ///
    /// ## Panics
    /// - Panics if `capacity == 0`.
    ///
    /// ## Concurrency
    /// - This method itself is not concurrent; it must be called before the
    ///   sequence is shared across threads.
    /// - Once constructed, the returned `Sequence` is safe to share and use
    ///   concurrently (subject to the container’s API contracts).
    ///
    /// ## Memory Semantics
    /// - Allocates one cache-line-aligned `Unit<T>` per slot.
    /// - Does **not** allocate any `T` values.
    /// - All memory is owned exclusively by the returned `Sequence`.
    ///
    /// ## Performance Characteristics
    /// - Time complexity: **O(capacity)**
    /// - Memory overhead: one atomic pointer per slot
    /// - No further allocations are required for insertion.
    ///
    /// ## Guarantees
    /// - `capacity()` will always return the value passed to this constructor.
    /// - `length()` will initially return `0`.
    ///
    /// ## Warnings
    /// - Capacity cannot be changed after construction.
    /// - Large capacities increase memory usage linearly.
    /// - Choosing an excessively small capacity may lead to frequent insertion
    ///   failures for random or checked writes.
    ///
    /// ## When to Use
    /// - When the maximum number of elements is known ahead of time.
    /// - When predictable memory usage is required.
    ///
    /// ## When Not to Use
    /// - When unbounded or dynamically growing storage is required.
    #[must_use = "Allocated Sequences must serve a purpose!"]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");

        let mut units = Vec::with_capacity(capacity.max(1));
        for _ in 0..capacity { units.push(Unit::new()); }

        Self {
            units: units.into_boxed_slice(),
            capacity,
            length: AtomicUsize::new(0)
        }
    }

    /// Creates a new `Sequence` by cloning the contents of another sequence.
    ///
    /// This function produces a *best-effort snapshot* of the source sequence.
    /// All observed elements are cloned into a newly allocated sequence with
    /// the same capacity.
    ///
    /// ## Behavior
    /// - Allocates a new `Sequence` with the same capacity as the source.
    /// - Iterates over the source sequence using `for_each`.
    /// - Clones each observed element into the corresponding index of the new sequence.
    /// - Empty slots in the source remain empty in the derivative.
    ///
    /// ## Concurrency
    /// - Lock-free with respect to the source sequence.
    /// - Safe to call concurrently with reads and writes on the source.
    /// - Does **not** provide a linearizable or atomic snapshot.
    ///
    /// ## Snapshot Semantics
    /// ⚠️ **Important**:
    /// - The resulting sequence may reflect a mixture of old and new values.
    /// - Elements inserted, removed, or rewritten during iteration may or may not
    ///   appear in the result.
    /// - No guarantee is made that the returned sequence corresponds to any
    ///   single point-in-time state of the source.
    ///
    /// This is a *weak snapshot*.
    ///
    /// ## Memory & Lifetime
    /// - All values in the returned sequence are freshly allocated clones.
    /// - The returned sequence shares **no memory** with the source.
    /// - Dropping either sequence does not affect the other.
    ///
    /// ## Type Constraints
    /// - Requires `T: Clone`.
    ///
    /// ## Performance
    /// - Time complexity: **O(capacity)**
    /// - Allocates memory proportional to the number of observed elements.
    /// - Performs one clone per observed element.
    ///
    /// ## Warnings
    /// - Do **not** rely on this method for consistency-sensitive operations
    ///   such as transactions or checkpoints.
    /// - If the source is being heavily mutated, the resulting sequence may be
    ///   sparse or partially populated.
    ///
    /// ## When to Use
    /// - Creating a read-only snapshot for analysis or debugging.
    /// - Transferring data between sequences without shared ownership.
    /// - When approximate consistency is acceptable.
    ///
    /// ## When Not to Use
    /// - When a strict or atomic snapshot is required.
    /// - When cloning `T` is expensive and contention is high.
    #[must_use = "Allocated Sequences must serve a purpose!"]
    pub fn from_sequence(sequence: Sequence<T>) -> Self
    where 
        T: Clone,
    {
        let derivative = Sequence::new(sequence.capacity().max(1));
        sequence.for_each(|index, value| {
            derivative.write(index, value.clone());
        });
        derivative
    }

    /// Creates a [`Sequence`] from a [`HashMap`] by extracting either its keys or values.
	///
	/// # Behavior
	/// - Consumes the provided `HashMap`.
	/// - Iterates over the map **once** and inserts elements sequentially starting at index `0`.
	/// - The resulting `Sequence` has a capacity equal to `hashmap.capacity().max(1)`.
	/// - The insertion order is **arbitrary** and depends on the hash map's internal iteration order.
	///
	/// # Return Value
	/// - Returns a [`HashMapToSequence`] enum that contains:
	///   - `FromKeys(Sequence<K>)` if `use_values == false`
	///   - `FromValues(Sequence<V>)` if `use_values == true`
	///
	/// # Concurrency
	/// - This function itself is **single-threaded**.
	/// - The returned `Sequence` supports concurrent access according to its guarantees.
	///
	/// # Consistency
	/// - The resulting `Sequence` is a **snapshot** of the `HashMap` at the time of conversion.
	/// - Subsequent modifications to the original map (which is consumed anyway) are irrelevant.
	///
	/// # Performance
	/// - Time complexity: **O(n)**
	/// - Allocations:
	///   - One allocation for the `Sequence` backing storage.
	///   - One allocation per inserted element.
	///
	/// # Warnings
	/// - **Ordering is not preserved**.
	/// - Capacity may exceed actual element count due to `HashMap` over-allocation.
	/// - If the map is empty, capacity is still forced to `1`.
	///
	/// # When to Use
	/// - When you need a lock-free, index-addressable container derived from a `HashMap`.
	///
	/// # When to Avoid
	/// - If deterministic iteration order is required.
    #[must_use = "Allocated Sequences must serve a purpose!"]
    pub fn from_hashmap<K, V>(hashmap: std::collections::HashMap<K, V>, use_values: bool) -> HashMapToSequence<K, V>
    where 
        K: Clone,
        V: Clone,
    {
        let mut index = 0;

        if use_values {
            let sequence = Sequence::new(hashmap.capacity().max(1));
            for (_, v) in hashmap {
                sequence.write(index, v);
                index += 1;
            }
            HashMapToSequence::FromValues(sequence)
        } else {
            let sequence = Sequence::new(hashmap.capacity().max(1));
            for (k, _) in hashmap {
                sequence.write(index, k);
                index += 1;
            }
            HashMapToSequence::FromKeys(sequence)
        }
    }

    /// Converts the sequence into a formatted `String` by concatenating elements with a separator.
	///
	/// # Behavior
	/// - Iterates over all **currently present** elements in the sequence.
	/// - Appends each element’s `Display` representation followed by `sep`.
	/// - Removes the trailing separator (if any) before returning.
	/// - Appends a closing `']'` character to the result.
	///
	/// # Ordering
	/// - Iteration order follows the **internal index order** of the sequence.
	/// - Empty slots are skipped.
	/// - No guarantees are made about semantic ordering (e.g., insertion order).
	///
	/// # Concurrency
	/// - This method is **lock-free**.
	/// - The output is a **best-effort snapshot**:
	///   - Elements concurrently inserted or removed may or may not appear.
	///   - No linearizability is guaranteed.
	///
	/// # Performance
	/// - Time complexity: **O(n)** where `n == capacity`
	/// - Allocations:
	///   - Grows a `String` dynamically (no preallocation).
	///   - Uses `format!` per element (relatively expensive).
	///
	/// # Warnings
	/// - The returned string always ends with `']'`, but **does not prepend `'['`**.
	/// - Concurrent mutation can lead to partial or mixed snapshots.
	/// - Using this in hot paths may be expensive due to repeated formatting.
	///
	/// # When to Use
	/// - Debugging, logging, or human-readable output.
	///
	/// # When to Avoid
	/// - Performance-sensitive paths.
	/// - When a stable or atomic snapshot is required.
    #[must_use = "String conversion is O(n), therefore must serve a purpose!"]
    pub fn to_string(&self, sep: &str) -> String
    where
        T: Display,
    {
        let mut result = String::new();
        self.for_each(|_, value| {
            result += &format!("{}{}", value, sep);
        });

        return result.trim_suffix(sep).to_string() + "]";
    }

	/// Returns the current number of occupied slots in the sequence.
	///
	/// # Behavior
	/// - Loads and returns the internal atomic length counter.
	/// - The value represents the number of slots that were **observed as occupied**
	///   at some point during concurrent execution.
	///
	/// # Concurrency
	/// - This method is **lock-free** and wait-free.
	/// - Uses `Ordering::Acquire` to ensure visibility of prior writes
	///   to elements that happened-before the corresponding length increment.
	///
	/// # Consistency
	/// - The returned value is **not linearizable** with respect to inserts/removals.
	/// - Under concurrent mutation, the value may be:
	///   - Slightly stale
	///   - Temporarily inconsistent with the actual number of non-null slots
	///
	/// # Performance
	/// - Single atomic load.
	/// - Inlined aggressively due to `#[inline(always)]`.
	///
	/// # Warnings
	/// - **Must not be used as a synchronization primitive.**
	/// - Do not assume `length()` precisely equals the number of readable elements
	///   during concurrent modification.
	///
	/// # When to Use
	/// - Fast emptiness checks.
	/// - Heuristic decisions (e.g., occupancy, load factor).
	///
	/// # When to Avoid
	/// - When an exact, linearizable element count is required.
	#[inline(always)]
	#[must_use = "Queried length must serve a purpose!"]
	pub fn length(&self) -> usize {
		self.length.load(Ordering::Acquire)
	}

	/// Returns the fixed capacity of the sequence.
	///
	/// # Behavior
	/// - Returns the total number of addressable slots in the sequence.
	/// - This value is fixed at construction time and never changes.
	///
	/// # Concurrency
	/// - This method is **thread-safe** and wait-free.
	/// - No atomic operations are required, as `capacity` is immutable.
	///
	/// # Consistency
	/// - The returned value is always exact and globally consistent.
	/// - Independent of concurrent inserts or removals.
	///
	/// # Performance
	/// - Constant-time, zero-cost accessor.
	/// - Always inlined due to `#[inline(always)]`.
	///
	/// # Warnings
	/// - Capacity does **not** imply occupancy.
	/// - A capacity greater than `length()` does not guarantee available slots
	///   under concurrent mutation.
	///
	/// # When to Use
	/// - Bounds checks.
	/// - Computing load factors or occupancy ratios.
	///
	/// # When to Avoid
	/// - Do not use capacity alone to infer available space in concurrent contexts.
	#[inline(always)]
	#[must_use = "Queried capacity must serve a purpose!"]
	pub fn capacity(&self) -> usize {
		self.capacity
	}

	/// Writes a value into the sequence at the given index.
	///
	/// # Semantics
	/// - Inserts `value` at `index`.
	/// - If the slot is empty, it is **filled**.
	/// - If the slot already contains a value, it is **overwritten**.
	///
	/// # Return Value
	/// - Returns `true` if the slot was previously empty.
	/// - Returns `false` if an existing value was overwritten.
	///
	/// # Panics
	/// - Panics if `index >= capacity()`.
	///
	/// # Concurrency
	/// - This method is **thread-safe**.
	/// - Uses an atomic swap with `Ordering::AcqRel` to ensure:
	///   - Proper publication of the newly written value.
	///   - Safe acquisition of the previously stored pointer.
	/// - Length updates use `Ordering::Relaxed` because slot ownership
	///   is already synchronized by the swap.
	///
	/// # Memory Safety
	/// - Values are heap-allocated and stored as raw pointers internally.
	/// - On overwrite, the old value is safely dropped.
	/// - No memory leaks occur as long as every stored pointer is eventually
	///   reclaimed via overwrite or sequence destruction.
	///
	/// # Performance
	/// - Constant-time operation (`O(1)`).
	/// - Lock-free and wait-free.
	///
	/// # Notes
	/// - This method does **not** check whether the slot was logically freed
	///   by another thread between operations; the swap is authoritative.
	/// - Prefer this method when index placement is deterministic.
	pub fn write(&self, index: usize, value: T) -> bool {
		assert!(index < self.capacity, "Index out of bounds");

		let new_pointer = Box::into_raw(Box::new(value));
		let old_pointer = self.units[index]
			.data
			.swap(new_pointer, Ordering::AcqRel);

		if old_pointer.is_null() {
			self.length.fetch_add(1, Ordering::Relaxed);
			true
		} else {
			unsafe { drop(Box::from_raw(old_pointer)); }
			false
		}
	}

	/// Writes a value into the sequence at the given index **only if the slot is empty**.
	///
	/// # Semantics
	/// - Attempts to insert `value` at `index`.
	/// - If the slot is empty (`null`), the value is inserted.
	/// - If the slot is already occupied, the insertion is rejected.
	/// - Existing values are **never overwritten**.
	///
	/// # Return Value
	/// - Returns `true` if the value was successfully inserted.
	/// - Returns `false` if the slot was already occupied.
	///
	/// # Panics
	/// - Panics if `index >= capacity()`.
	///
	/// # Concurrency
	/// - This method is **thread-safe**.
	/// - Uses `compare_exchange` with:
	///   - `Ordering::AcqRel` on success to publish the new value.
	///   - `Ordering::Acquire` on failure to observe an existing value.
	/// - Guarantees that **at most one thread** can successfully insert
	///   into a given slot.
	///
	/// # Memory Safety
	/// - Allocates the value on the heap before attempting insertion.
	/// - If insertion fails, the newly allocated value is immediately dropped.
	/// - On success, ownership of the allocation is transferred to the slot.
	///
	/// # Performance
	/// - Constant-time (`O(1)`).
	/// - Lock-free.
	/// - May incur allocation cost even on failure.
	///
	/// # Warnings
	/// - Because allocation happens *before* the CAS, high contention on the
	///   same index may lead to repeated allocate–free cycles.
	/// - This method does **not** retry on CAS failure.
	///
	/// # Notes
	/// - Prefer this method when overwriting must be prevented.
	/// - If retry semantics are required, wrap this call in a loop or
	///   provide a separate `write_checked_retry` variant.
	pub fn write_checked(&self, index: usize, value: T) -> bool {
		assert!(index < self.capacity, "Index out of bounds");

		let new_pointer = Box::into_raw(Box::new(value));

		match self.units[index].data.compare_exchange(
			ptr::null_mut(),
			new_pointer,
			Ordering::AcqRel,
			Ordering::Acquire,
		) {
			Ok(_) => {
				self.length.fetch_add(1, Ordering::Relaxed);
				true
			}
			Err(_) => {
				unsafe { drop(Box::from_raw(new_pointer)); }
				false
			}
		}
	}

	/// Attempts to write a value into a **random empty slot** in the sequence.
	///
	/// # Semantics
	/// - Starts probing from a pseudo-random index.
	/// - Linearly scans all slots (wrapping around) until:
	///   - An empty slot is found and filled, or
	///   - All slots are observed to be occupied.
	///
	/// # Return Value
	/// - Returns `Some(index)` if the value was successfully inserted.
	/// - Returns `None` if the sequence is full.
	///
	/// # Concurrency
	/// - This method is **thread-safe**.
	/// - Uses `compare_exchange` with:
	///   - `Ordering::AcqRel` on success to publish the value.
	///   - `Ordering::Acquire` on failure to observe an existing value.
	/// - Multiple threads may race for the same slot, but at most one
	///   succeeds per slot.
	///
	/// # Memory Safety
	/// - The value is heap-allocated **once** and reused across retries.
	/// - On CAS failure, ownership of the allocation is reclaimed and reused.
	/// - On success, ownership is transferred to the slot.
	/// - On total failure (sequence full), the value is dropped exactly once.
	///
	/// # Performance
	/// - Worst-case complexity: `O(capacity)`
	/// - Lock-free, but **not wait-free**.
	/// - Randomized starting point helps reduce contention under parallel use.
	///
	/// # Panics
	/// - Never panics.
	///
	/// # Warnings
	/// - This method performs a full scan when the sequence is nearly full,
	///   which may be expensive under high contention.
	/// - No fairness guarantees: one thread may repeatedly lose CAS races.
	///
	/// # Notes
	/// - This is effectively a lock-free “open addressing” insert.
	/// - Prefer `write_checked(index, value)` when the target index is known.
	/// - Prefer this method when index placement does not matter and
	///   probabilistic load balancing is acceptable.
	pub fn write_random(&self, value: T) -> Option<usize> {
		let start = RandomIndex::new(self.capacity);
		let mut boxed = Box::new(value);

		for offset in 0..self.capacity {
			let index = (start + offset) % self.capacity;
			let pointer = Box::into_raw(boxed);

			match self.units[index].data.compare_exchange(
				ptr::null_mut(),
				pointer,
				Ordering::AcqRel,
				Ordering::Acquire,
			) {
				Ok(_) => {
					self.length.fetch_add(1, Ordering::Relaxed);
					return Some(index);
				}
				Err(_) => {
					// CAS failed, reclaim ownership and try next slot
					boxed = unsafe { Box::from_raw(pointer) };
				}
			}
		}

		// Sequence is full
		drop(boxed);
		None
	}

	/// Removes and returns the value stored at the given index.
	///
	/// # Semantics
	/// - Atomically clears the slot at `index`.
	/// - If the slot was empty, no action is taken.
	/// - If the slot contained a value, ownership of that value is returned.
	///
	/// # Return Value
	/// - Returns `Some(T)` if a value was present.
	/// - Returns `None` if the slot was already empty.
	///
	/// # Panics
	/// - Panics if `index >= capacity()`.
	///
	/// # Concurrency
	/// - This method is **thread-safe**.
	/// - Uses an atomic `swap` with `Ordering::AcqRel` to:
	///   - Acquire visibility of the removed value.
	///   - Release the slot as empty (`null`) to other threads.
	/// - Ensures that at most one thread can successfully remove
	///   a given value.
	///
	/// # Memory Safety
	/// - Internally stores values as heap-allocated pointers.
	/// - On successful removal, ownership of the allocation is
	///   transferred back to the caller.
	/// - No double-free occurs: the slot is cleared before the value
	///   is reconstructed.
	///
	/// # Performance
	/// - Constant-time (`O(1)`).
	/// - Lock-free and wait-free.
	///
	/// # Warnings
	/// - ⚠️ **ABA hazard**: If another thread removes a value and later
	///   inserts a new value at the same index, concurrent observers
	///   cannot distinguish old vs new generations using the pointer alone.
	///   This is acceptable for simple slot ownership but unsafe for
	///   long-lived external references.
	/// - The returned value is a **snapshot**; it is no longer tracked
	///   by the sequence.
	///
	/// # Notes
	/// - This is the logical inverse of `write()` / `write_checked()`.
	/// - Safe to call concurrently with writers.
	/// - After removal, the slot immediately becomes available for reuse.
	pub fn free(&self, index: usize) -> Option<T> {
		assert!(index < self.capacity, "Index out of bounds");

		let old_pointer = self.units[index]
			.data
			.swap(ptr::null_mut(), Ordering::AcqRel);

		if old_pointer.is_null() {
			None
		} else {
			self.length.fetch_sub(1, Ordering::Relaxed);
			unsafe { Some(*Box::from_raw(old_pointer)) }
		}
	}

	/// Removes and returns a value from a **randomly chosen occupied slot**.
	///
	/// # Semantics
	/// - Starts probing from a pseudo-random index.
	/// - Linearly scans the sequence (with wrap-around) until:
	///   - A non-empty slot is found and cleared, or
	///   - All slots are observed to be empty.
	///
	/// # Return Value
	/// - Returns `Some(T)` if a value was successfully removed.
	/// - Returns `None` if the sequence is empty.
	///
	/// # Concurrency
	/// - This method is **thread-safe**.
	/// - Uses an atomic `swap` with `Ordering::AcqRel` to:
	///   - Acquire ownership of the removed value.
	///   - Release the slot as empty to other threads.
	/// - Guarantees that at most one thread can remove a given value.
	///
	/// # Memory Safety
	/// - Values are stored as heap-allocated pointers internally.
	/// - On successful removal, ownership of the allocation is transferred
	///   back to the caller.
	/// - No double-free or use-after-free occurs under correct usage.
	///
	/// # Performance
	/// - Worst-case complexity: `O(capacity)`
	/// - Lock-free, but **not wait-free**.
	/// - Randomized probing reduces contention under concurrent access.
	///
	/// # Panics
	/// - Never panics.
	///
	/// # Warnings
	/// - ⚠️ **ABA hazard**: As with `free(index)`, pointer reuse at the same
	///   index cannot distinguish between different generations of values.
	/// - No fairness guarantees: one thread may repeatedly miss removals
	///   under heavy contention.
	///
	/// # Notes
	/// - This is the randomized counterpart to `free(index)`.
	/// - Safe to call concurrently with writers and other removers.
	/// - Slot reuse is immediate after removal.
	pub fn free_random(&self) -> Option<T> {
		let start = RandomIndex::new(self.capacity);

		for offset in 0..self.capacity {
			let index = (start + offset) % self.capacity;
			let old_pointer = self.units[index]
				.data
				.swap(ptr::null_mut(), Ordering::AcqRel);

			if !old_pointer.is_null() {
				self.length.fetch_sub(1, Ordering::Relaxed);
				unsafe { return Some(*Box::from_raw(old_pointer)); }
			}
		}

		None
	}

    /// Executes a closure on the value stored at the given index **without removing it**.
	///
	/// # Semantics
	/// - Atomically loads the pointer at `index`.
	/// - If the slot is empty, the closure is not executed.
	/// - If the slot contains a value, a shared reference is passed to `usage`.
	/// - The value remains stored in the sequence after the call.
	///
	/// # Return Value
	/// - Returns `Some(R)` containing the closure’s result if a value was present.
	/// - Returns `None` if the slot was empty.
	///
	/// # Panics
	/// - Panics if `index >= capacity()`.
	///
	/// # Concurrency
	/// - This method is **thread-safe**.
	/// - Uses `Ordering::Acquire` to ensure the loaded value is fully
	///   initialized before being observed.
	/// - Concurrent writers or removers may modify the slot immediately
	///   after the load.
	///
	/// # Memory Safety
	/// - The reference passed to `usage` is **ephemeral** and valid only
	///   for the duration of the closure call.
	/// - If another thread calls `free()` concurrently, the value may be
	///   deallocated **after** the load but **before** or **during** the
	///   closure execution, leading to **use-after-free**.
	///
	/// # Warnings
	/// - ⚠️ This method is **not memory-safe under concurrent removal**
	///   unless external synchronization guarantees that the slot will
	///   not be freed while `usage` executes.
	/// - Do **not** store or leak the reference beyond the closure.
	///
	/// # Performance
	/// - Constant-time (`O(1)`).
	/// - Lock-free.
	///
	/// # Notes
	/// - Safe for read-only access when no concurrent `free()` can occur.
	/// - If safe concurrent reads are required, consider:
	///   - Reference counting
	///   - Epoch-based reclamation
	///   - Hazard pointers
	pub fn with<F, R>(&self, index: usize, usage: F) -> Option<R>
	where
		F: FnOnce(&T) -> R,
	{
		assert!(index < self.capacity, "Index out of bounds");

		let pointer = self.units[index].data.load(Ordering::Acquire);

		if pointer.is_null() {
			None
		} else {
			unsafe { Some(usage(&*pointer)) }
		}
	}

	/// Executes a closure on a **cloned copy** of the value at the given index.
	///
	/// # Semantics
	/// - Attempts to read the value at `index`.
	/// - If the slot is empty, the closure is not executed.
	/// - If the slot contains a value, it is cloned and passed to `usage`.
	/// - The original value remains stored in the sequence.
	///
	/// # Return Value
	/// - Returns `Some(R)` containing the closure’s result if a value was present.
	/// - Returns `None` if the slot was empty.
	///
	/// # Panics
	/// - Panics if `index >= capacity()`.
	///
	/// # Concurrency
	/// - This method is **thread-safe** with respect to mutation.
	/// - Relies on `with()` for the initial read, using `Ordering::Acquire`
	///   to observe a fully initialized value.
	///
	/// # Memory Safety
	/// - The cloned value is owned and independent of the sequence.
	/// - After cloning, the original slot may be concurrently freed or
	///   overwritten without affecting the closure execution.
	///
	/// # Warnings
	/// - ⚠️ This method inherits the **soundness preconditions** of `with()`
	///   during the cloning step:
	///   - If another thread calls `free()` concurrently, the value may be
	///     deallocated while `clone()` is executing, leading to
	///     **use-after-free**.
	/// - Cloning does **not** retroactively make the read safe.
	///
	/// # Performance
	/// - Constant-time for the lookup plus the cost of `T::clone()`.
	/// - Lock-free.
	///
	/// # Notes
	/// - This is safer than `with()` *only after* the clone succeeds.
	/// - For true concurrent safety, the read itself must be protected
	///   (e.g., epoch-based reclamation).
	pub fn with_cloned<F, R>(&self, index: usize, usage: F) -> Option<R>
	where
		F: FnOnce(T) -> R,
		T: Clone,
	{
		let value = self.with(index, |value| value.clone());

		if let Some(value) = value {
			Some(usage(value))
		} else {
			None
		}
	}

    pub fn is_written(&self, index: usize) -> bool {
        // Check if the slot at the specified index is occupied.
        assert!(index < self.capacity, "Index out of bounds");
        !self.units[index].data.load(Ordering::Acquire).is_null()
    }

    pub fn rewrite<F>(&self, index: usize, mut closure: F) -> bool
    where 
        F: FnMut(&T) -> T,
    {
        // Update the value at an index using a function
        // Returns true if update succeeded, false if slot was empty.
        assert!(index < self.capacity, "Index out of bounds");

        loop {
            let current_pointer = self.units[index].data.load(Ordering::Acquire);
            if current_pointer.is_null() { return false; }

            let new_value = unsafe { closure(&*current_pointer) };
            let new_pointer = Box::into_raw(Box::new(new_value));

            match self.units[index].data.compare_exchange(current_pointer, new_pointer, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    // Success - free the old value
                    unsafe { drop!(Box::from_raw(current_pointer)); }
                    return true;
                },
                Err(_) => {
                    // CAS failed - free the new value and retry
                    unsafe { drop!(Box::from_raw(new_pointer)); }
                    std::hint::spin_loop();
                }
            }
        }
    }

    pub fn find<F, R>(&self, predicate: F) -> Option<R>
    where
        F: Fn(&T) -> Option<R>,
    {
        // find the first value matching the predicate
        for unit in self.units.iter() {
            let pointer = unit.data.load(Ordering::Acquire);
            if !pointer.is_null() {
                unsafe {
                    if let Some(result) = predicate(&*pointer) {
                        return Some(result);
                    }
                }
            }
        }
        None
    }

    pub fn find_all<F, R>(&self, predicate: F) -> Sequence<R>
    where 
        F: Fn(&T) -> Option<R>,
    {
        // find the values matching the predicate.
        let mut vector = vec![];

        for unit in self.units.iter() {
            let pointer = unit.data.load(Ordering::Acquire);
            if !pointer.is_null() {
                unsafe {
                    if let Some(result) = predicate(&*pointer) {
                        vector.push(result);
                    }
                }
            }
        }

        let sequence = Sequence::new(vector.len().max(1));
        let mut index = 0;
        for element in vector {
            sequence.write(index, element);
            index += 1;
        }

        sequence
    }

    pub fn for_each<F>(&self, mut closure: F)
    where
        F: FnMut(usize, &T),
    {
        // Iterate over all values with their indices
        for (index, unit) in self.units.iter().enumerate() {
            let pointer = unit.data.load(Ordering::Acquire);
            if !pointer.is_null() {
                unsafe { closure(index, &*pointer); }
            }
        }
    }

    pub fn to_vec(&self) -> Vec<T>
    where 
        T: Clone,
    {
        // create a snapshot of all values (clone).
        let mut result = Vec::new();

        for unit in self.units.iter() {
            let pointer = unit.data.load(Ordering::Acquire);
            if !pointer.is_null() {
                unsafe { result.push((*pointer).clone()); }
            }
        }

        result
    }

    pub fn drain(&self) -> Sequence<T> {
        // drain all values from the list (removes and returns them)
        let mut vector = Vec::new();

        for unit in self.units.iter() {
            let old_pointer = unit.data.swap(ptr::null_mut(), Ordering::AcqRel);
            if !old_pointer.is_null() {
                self.length.fetch_sub(1, Ordering::Relaxed);
                unsafe { vector.push(*Box::from_raw(old_pointer)); }
            }
        }

        vector.into()
    }

    pub fn drain_with<F>(&self, mut predicate: F) -> Sequence<T>
    where 
        F: FnMut(&T) -> bool,
    {
        // drain all the values from the list that gives True for predicate.
        // dangerous in concurrent scenarios
        let mut vector = Vec::new();

        for unit in self.units.iter() {
            let old_pointer = unit.data.load(Ordering::Acquire);
            unsafe {
                if !old_pointer.is_null() && predicate(&*old_pointer) {
                    let old_pointer = unit.data.swap(ptr::null_mut(), Ordering::AcqRel);
                    self.length.fetch_sub(1, Ordering::Relaxed);
                    vector.push(*Box::from_raw(old_pointer));
                }
            }
        }

        vector.into()
    }

    pub fn clear(&self) {
        // clear all values from the list.
        for unit in self.units.iter() {
            let old_pointer = unit.data.swap(ptr::null_mut(), Ordering::AcqRel);
            if !old_pointer.is_null() {
                unsafe { drop!(Box::from_raw(old_pointer)); }
            }
        }
        self.length.store(0, Ordering::Relaxed);
    }

    pub fn retain<F>(&self, mut predicate: F)
    where
        F: FnMut(&T) -> bool,
    {
        // remove all values that don't match the predicate.
        for unit in self.units.iter() {
            loop {
                let current_pointer = unit.data.load(Ordering::Acquire);
                if current_pointer.is_null() { break; }

                let should_keep = unsafe { predicate(&*current_pointer) };
                if should_keep { break; }

                match unit.data.compare_exchange(current_pointer, ptr::null_mut(), Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => {
                        self.length.fetch_sub(1, Ordering::Relaxed);
                        unsafe { drop!(Box::from_raw(current_pointer)); }
                        break;
                    },
                    Err(_) => {
                        std::hint::spin_loop();
                    }
                }
            }
        }
    }

    pub fn write_batch<I>(&self, iter: I) -> usize
    where 
        I: IntoIterator<Item = T>,
    {
        // Batch insert multiple values at random positions
        // Return the number of successfully inserted values.
        let mut inserted = 0;
        for value in iter {
            if self.write_random(value).is_some() {
                inserted += 1;
            }
        }
        inserted
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.length() == 0
    }

    pub fn contains<F>(&self, predicate: F) -> bool
    where 
        F: Fn(&T) -> bool,
    {
        self.find(|v| predicate(v).then_some(())).is_some()
    }

    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let pointer = self.units[index].data.load(Ordering::Acquire);
        if pointer.is_null() { None } else { unsafe { Some(&*pointer) } }
    }

    pub fn get_cloned(&self, index: usize) -> Option<T>
    where 
        T: Clone,
    {
        self.with(index, |v| v.clone())
    }

    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.units.iter().filter_map(|u| {
            let p = u.data.load(Ordering::Acquire);
            if p.is_null() { None } else { unsafe { Some(&*p) } }
        })
    }

    pub fn try_insert(&self, index: usize, value: T) -> anyhow::Result<()> {
        if self.write_checked(index, value) { Ok(()) } else { Err(anyhow::anyhow!("Insert Failed!")) }
    }

    pub fn replace(&self, index: usize, value: T) -> Option<T> {
        assert!(index < self.capacity, "Index out of bounds");
        let new_pointer = Box::into_raw(Box::new(value));
        let old_pointer = self.units[index].data.swap(new_pointer, Ordering::AcqRel);

        if old_pointer.is_null() {
            self.length.fetch_add(1, Ordering::Relaxed);
            None
        } else {
            unsafe { Some(*Box::from_raw(old_pointer)) }
        }
    }

    pub fn extend<I>(&self, with: I)
    where 
        I: IntoIterator<Item = T>,
    {
        for v in with {
            // attempt to insert at a random location
            // if cannot be inserted, break.
            // insert as many as possible.
            if self.write_random(v).is_none() {
                break;
            }
        }
    }

    pub fn occupancy(&self) -> f32 {
        self.length() as f32 / self.capacity as f32
    }

    pub fn consistent_length(&self) -> usize {
        loop {
            let before = self.length.load(Ordering::Acquire);
            std::sync::atomic::fence(Ordering::SeqCst);
            let after = self.length.load(Ordering::Acquire);
            if before == after {
                return before;
            }
        }
    }

    pub fn debug(&self)
    where 
        T: Display,
    {
        println!("{}", self.to_string(","));
    }

    pub fn retain_indexed<F>(&self, mut predicate: F)
    where 
        F: FnMut(usize, &T) -> bool,
    {
        for (i, unit) in self.units.iter().enumerate() {
            let p = unit.data.load(Ordering::Acquire);
            if !p.is_null() && !unsafe { predicate(i, &*p) } {
                if unit.data.compare_exchange(p, ptr::null_mut(), Ordering::AcqRel, Ordering::Acquire).is_ok() {
                    self.length.fetch_sub(1, Ordering::Relaxed);
                    unsafe { drop!(Box::from_raw(p)); }
                }
            }
        }
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.length() == self.capacity
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.length()
    }

    pub fn clear_lenient(&self) {
        // does not guarantee that all values are dropped immediately.
        for unit in self.units.iter() {
            let _ = unit.data.swap(ptr::null_mut(), Ordering::Release);
        }
        self.length.store(0, Ordering::Relaxed);
    }

    pub fn write_or_replace(&self, index: usize, value: T) {
        let _ = self.replace(index, value);
    }

    pub fn update_or_insert<F>(&self, index: usize, value: T, update: F)
    where 
        F: FnMut(&T) -> T,
    {
        if !self.rewrite(index, update) {
            let _ = self.write_checked(index, value);
        }
    }

    pub fn remove_where<F>(&self, predicate: F) -> Option<T>
    where 
        F: Fn(&T) -> bool,
    {
        for unit in self.units.iter() {
            let p = unit.data.load(Ordering::Acquire);
            if !p.is_null() && unsafe { predicate(&*p) } {
                let old_pointer = unit.data.swap(ptr::null_mut(), Ordering::AcqRel);
                if !old_pointer.is_null() {
                    self.length.fetch_sub(1, Ordering::Relaxed);
                    unsafe { return Some(*Box::from_raw(old_pointer)); }
                }
            }
        }
        None
    }

    pub fn find_index<F>(&self, predicate: F) -> Option<usize>
    where 
        F: Fn(&T) -> bool,
    {
        for (i, unit) in self.units.iter().enumerate() {
            let p = unit.data.load(Ordering::Acquire);
            if !p.is_null() && unsafe { predicate(&*p) } {
                return Some(i);
            }
        }
        None
    }

    pub fn stats(&self) -> SequenceStats {
        SequenceStats { capacity: self.capacity, length: self.length(), occupancy: self.occupancy() }
    }

    #[cfg(debug_assertions)]
    pub fn assert_consistent(&self) {
        let mut count = 0;
        for u in self.units.iter() {
            if !u.data.load(Ordering::Acquire).is_null() {
                count += 1;
            }
        }
        assert_eq!(count, self.len());
    }

    pub fn random(&self) -> Option<&T> {
        let start = RandomIndex::new(self.capacity);
        for offset in 0..self.capacity {
            let index = (start + offset) % self.capacity;
            if let Some(value) = self.get(index) {
                return Some(value);
            }
        }
        None
    }

    pub fn min_by<F>(&self, mut cmp: F) -> Option<T>
    where 
        F: FnMut(&T, &T) -> std::cmp::Ordering,
        T: Clone,
    {
        let mut best: Option<T> = None;

        for unit in self.units.iter() {
            let ptr = unit.data.load(Ordering::Acquire);
            if ptr.is_null() {
                continue;
            }

            unsafe {
                let v = &*ptr;
                match &best {
                    None => best = Some(v.clone()),
                    Some(cur) => {
                        if cmp(v, cur).is_lt() {
                            best = Some(v.clone());
                        }
                    }
                }
            }
        }

        best
    }

    pub fn max_by<F>(&self, mut cmp: F) -> Option<T>
    where
        F: FnMut(&T, &T) -> std::cmp::Ordering,
        T: Clone,
    {
        let mut best: Option<T> = None;

        for unit in self.units.iter() {
            let ptr = unit.data.load(Ordering::Acquire);
            if ptr.is_null() {
                continue;
            }

            unsafe {
                let value = &*ptr;
                match &best {
                    None => {
                        best = Some(value.clone());
                    }
                    Some(current) => {
                        if cmp(value, current).is_gt() {
                            best = Some(value.clone());
                        }
                    }
                }
            }
        }

        best
    }

}

// Implement Send and Sync
unsafe impl<T: Send> Send for Sequence<T> {}
unsafe impl<T: Send> Sync for Sequence<T> {}

impl<T> Drop for Sequence<T> {
    fn drop(&mut self) {
        for unit in self.units.iter() {
            let pointer = unit.data.load(Ordering::Acquire);
            if !pointer.is_null() {
                unsafe { drop!(Box::from_raw(pointer)); }
            }
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Sequence<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T> Default for Sequence<T> {
    fn default() -> Self {
        Self::new(1)
    }
}

impl<'a, T> IntoIterator for &'a Sequence<T> {
    type Item = &'a T;
    type IntoIter = Box<dyn Iterator<Item = &'a T> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl<T> Index<usize> for Sequence<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if let Some(value) = self.get(index) {
            return value;
        } else {
            let message = format!("Index \'{}\' cannot be accessed via square brackets as it is empty.", index);
            panic!("{}", message);
        }
    }
}

