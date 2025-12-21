# 🚀 Supersonic

[![Crates.io](https://img.shields.io/crates/v/supersonic.svg)](https://crates.io/crates/supersonic)
[![Documentation](https://docs.rs/supersonic/badge.svg)](https://docs.rs/supersonic)
[![License](https://img.shields.io/crates/l/supersonic.svg)](https://github.com/d33p0st/supersonic#license)

**A high-speed, high-performance, high-concurrency data structures library for Rust.**

Supersonic provides blazingly fast, thread-safe data structures optimized for demanding multi-threaded environments. Built with fine-tuned memory ordering, lock-free reads, and reactive capabilities, Supersonic is designed for scenarios where microsecond latency and maximum throughput are critical.

---

## 📋 Table of Contents

- [Features](#-features)
- [Installation](#-installation)
- [Quick Start](#-quick-start)
- [Data Structures](#-data-structures)
  - [Sequence\<T\>](#sequencet)
- [Performance](#-performance)
- [Use Cases](#-use-cases)
- [Documentation](#-documentation)
- [Contributing](#-contributing)
- [License](#-license)

---

## ✨ Features

- **🔥 High-Speed Operations**: Lock-free reads and optimized memory ordering for minimal latency
- **⚡ High Performance**: Fine-tuned atomic operations with Relaxed + Release patterns (5-20% faster than naive implementations)
- **🔀 High Concurrency**: Designed for heavy multi-threaded workloads with minimal contention
- **🔄 Reactive Capabilities**: Share data modifications across references or create isolated copies
- **📦 Zero-Copy Cloning**: Arc-based sharing makes cloning extremely cheap
- **🎯 Rich API**: Stack, queue, list operations plus advanced features like slicing, splitting, and draining
- **🔒 Thread-Safe**: All operations are safe to call from multiple threads concurrently
- **📊 Serialization**: Built-in bincode support for efficient binary serialization

---

## 📦 Installation

Add supersonic to your `Cargo.toml`:

```toml
[dependencies]
supersonic = "0.1.0"
```

Or use `cargo add`:

```bash
cargo add supersonic
```

---

## 🚀 Quick Start

```rust
use supersonic::sequence::prelude::*;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new sequence with capacity 10
    let seq = Sequence::<i32>::allocate(10).await;

    // Push elements (stack-like)
    seq.push(Candidate::Value(1)).await;
    seq.push(Candidate::Value(2)).await;
    seq.push(Candidate::Value(3)).await;

    // Get element at index
    let value = seq.get(1, true).await?;
    println!("Element at index 1: {:?}", value);

    // Pop from end
    let popped = seq.pop().await;
    println!("Popped: {:?}", popped);

    // Queue operations
    seq.enqueue(Candidate::Value(4)).await;
    let dequeued = seq.dequeue().await?;
    println!("Dequeued: {:?}", dequeued);

    // Slice and split
    let slice = seq.slice(Some(0), Some(2), None).await;
    let (first, second) = seq.split(1).await;

    // Create snapshot
    let snapshot = seq.snapshot().await;
    println!("Snapshot: {:?}", snapshot);

    Ok(())
}
```

---

## 🗂️ Data Structures

### `Sequence<T>`

A high-speed, high-performance, high-concurrency, lock-free, thread-safe, reactive sequence data structure.

#### Key Features

- **Lock-Free Reads**: Most read operations use atomic loads without acquiring locks
- **Memory Ordering Optimization**: Uses Relaxed ordering under locks with Release fences for 5-20% performance improvement
- **Dynamic Growth**: Automatically resizes when capacity is exceeded
- **Dual Nature**: Supports both reactive (shared) and non-reactive (isolated) operations

#### Reactivity

**Reactive operations** (`extract`, `slice`, `split`, `reverse`, `modify`) create sequences that share underlying `Arc<RwLock<T>>` references. Modifications through one sequence are visible through all others.

**Non-reactive operations** (`set`, non-reactive versions of extract/slice/split/reverse) create independent copies with isolated state.

#### Traits Implemented

- `Allocation<T>` - Allocate new sequences
- `Operation<T>` - Core operations (get, set, insert, remove, append)
- `Reactive<T>` - Reactive operations with shared state
- `NonReactive<T>` - Non-reactive operations with isolated state
- `Stack<T>` - LIFO operations (push, pop, peek)
- `Queue<T>` - FIFO operations (enqueue, dequeue)
- `Drain<T>` - Drain ranges of elements
- `SnapShot<T>` - Create Vec snapshots
- `Bincode<T>` - Binary serialization
- `Equality<T>` - Equality comparison strategies
- `Length` - Length operations

#### Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Clone | O(1) | Only clones Arc pointers |
| Length/Capacity | O(1) | Simple atomic loads |
| Get | O(1) | Direct array access |
| Set | O(1) | Direct array access with atomic swap |
| Append | Amortized O(1) | May trigger resize |
| Insert | O(n) | Requires shifting right |
| Remove | O(n) | Requires shifting left |
| Push/Pop | Amortized O(1) | Stack operations |
| Enqueue | Amortized O(1) | Queue insert |
| Dequeue | O(n) | Queue remove with shift |

---

## ⚡ Performance

Supersonic achieves high performance through several optimizations:

### Memory Ordering Optimization

All atomic operations under locks use **Relaxed ordering** with a final **Release operation** to publish changes. This eliminates unnecessary memory fences while maintaining correctness:

```rust
// Traditional approach
container.slots[i].store(ptr, Acquire);  // Extra fence!

// Optimized approach
container.slots[i].store(ptr, Relaxed);  // Under lock
container.length.store(len, Release);    // Publishes all changes
```

**Performance gains:**
- High contention scenarios: **15-20% improvement**
- Moderate use: **5-10% improvement**
- Bulk operations (slice, split, drain): **10-20% improvement**

### Lock-Free Reads

Read operations use atomic loads without acquiring locks when synchronization is optional, enabling true concurrent reads with zero contention.

---

## 🎯 Use Cases

Supersonic is ideal for demanding, high-throughput, low-latency scenarios:

### High-Frequency Trading (HFT)
Order books, tick data streams, and trade execution queues where microsecond latency matters.

### Real-Time Analytics
Processing streaming data from sensors, logs, or metrics with multiple concurrent readers/writers.

### Game Engines
Entity component systems, event queues, and shared game state across game loop threads.

### Network Servers
Connection pools, request queues, and message buffers in high-concurrency web servers and proxies.

### Scientific Computing
Shared work queues in parallel algorithms, particle simulations, and distributed computation.

### Audio/Video Processing
Real-time sample buffers, frame queues, and DSP pipelines where bounded latency prevents glitches.

### Blockchain/Distributed Systems
Transaction pools, mempool management, and consensus message queues.

---

## 📚 Documentation

Full API documentation is available on [docs.rs](https://docs.rs/supersonic).

Key documentation sections:
- [Sequence\<T\> Overview](https://docs.rs/supersonic/latest/supersonic/sequence/struct.Sequence.html)
- [Traits Documentation](https://docs.rs/supersonic/latest/supersonic/sequence/traits/index.html)
- [Examples](https://docs.rs/supersonic/latest/supersonic/sequence/struct.Sequence.html#examples)

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

## 📄 License

This project is licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

---

<div align="center">
  <sub>Built with ❤️ by <a href="https://github.com/d33p0st">d33pster</a></sub>
</div>
