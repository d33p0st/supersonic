
use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc};
use std::mem::MaybeUninit;
use std::cell::UnsafeCell;

#[repr(align(64))]
struct Slot<T> {
    ready: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for Slot<T> {}
unsafe impl<T: Send> Sync for Slot<T> {}

pub struct List<T> {
    slots: Box<[Slot<T>]>,
    write_index: AtomicUsize,
    capacity: usize,
}

impl <T> List<T> {
    pub async fn allocate(capacity: usize) -> Arc<Self> {
        Arc::new(Self::allocate_raw(capacity).await)
    }

    pub async fn allocate_raw(capacity: usize) -> Self {
        let mut vector = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            vector.push(Slot { ready: AtomicBool::new(false), value: UnsafeCell::new(MaybeUninit::uninit()), });
        }

        Self {
            slots: vector.into_boxed_slice(),
            write_index: AtomicUsize::new(0),
            capacity,
        }
    }

    #[inline(always)]
    pub async fn reserve(&self, n: usize) -> Option<usize> {
        let start = self.write_index.fetch_add(n, Ordering::Relaxed);
        if start + n > self.capacity { return None; }
        Some(start)
    }

    #[inline(always)]
    pub async unsafe fn write(&self, index: usize, value: T) {
        let slot = &self.slots[index];
        unsafe {
            (*slot.value.get()).as_mut_ptr().write(value);
        }
        slot.ready.store(true, Ordering::Release);
    }

    #[inline(always)]
    pub async unsafe fn get(&self, index: usize) -> Option<&T> {
        let slot = &self.slots[index];

        if !slot.ready.load(Ordering::Acquire) {
            return None;
        }

        unsafe {
            Some(&*(*slot.value.get()).as_ptr())
        }
    }
}