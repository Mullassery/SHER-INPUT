//! Lock-free, interrupt-safe SPSC ring buffer for high-frequency input events.
//!
//! This buffer is designed for single-producer/single-consumer handoff (e.g. ISR or
//! hardware reader thread -> compositor/service thread). It stores raw events without
//! heap allocation in the hot path and avoids locks. The canonical [`crate::event::InputEvent`]
//! is richer (heap-allocated strings) and not suitable for ISR/zero-copy ring storage;
//! use [`RawInputEvent`] here.
//!
//! # Safety / invariants
//! - Capacity MUST be a power of two.
//! - Only one producer may call [`InputRingBuffer::push`] concurrently/serially with respect
//!   to other producers; only one consumer may call [`InputRingBuffer::pop`].
//! - [`RawInputEvent`] must be [`Copy`] (or trivially movable). Do not rely on destructors
//!   running in the producer on push failure in an ISR.
//! - Indices use wrapping arithmetic with masking; the buffer always leaves one slot empty
//!   to distinguish full from empty (usable capacity = `capacity() - 1`).

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Raw hardware/input sample suitable for lock-free transfer across a ring buffer.
///
/// This is deliberately minimal and `#[repr(C)]` for predictable layout. It does not
/// include device routing or the full SHER input metadata (use [`crate::event::InputEvent`]
/// after normalization).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct RawInputEvent {
    pub event_type: u16, // EV_KEY=1, EV_REL=2, EV_ABS=3
    pub code: u16,
    pub value: i32,
    pub timestamp: u64,
}

/// Error returned when the ring buffer is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushError<T> {
    /// The buffer has no free slots; the event is returned unchanged.
    Full(T),
}

impl<T> std::fmt::Display for PushError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushError::Full(_) => write!(f, "ring buffer full"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for PushError<T> {}

/// Lock-free SPSC ring buffer.
pub struct InputRingBuffer<T, const N: usize> {
    buffer: UnsafeCell<[MaybeUninit<T>; N]>,
    head: AtomicUsize, // written by producer
    tail: AtomicUsize, // written by consumer
}

impl<T, const N: usize> InputRingBuffer<T, N> {
    const MASK: usize = if N == 0 { 0 } else { N - 1 };

    const fn assert_power_of_two() {
        assert!(
            N != 0 && (N & (N - 1)) == 0,
            "ring buffer size must be power of two"
        );
    }

    /// Create a new empty ring buffer.
    #[allow(clippy::declare_interior_mutable_const)]
    pub const fn new() -> Self {
        Self::assert_power_of_two();
        // SAFETY: MaybeUninit is valid to hold uninitialized values.
        let buffer = [const { MaybeUninit::uninit() }; N];
        Self {
            buffer: UnsafeCell::new(buffer),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Total capacity of the buffer (including the one slot reserved to distinguish
    /// full from empty). Usable capacity is `capacity() - 1`.
    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Maximum number of elements that can be stored without blocking/overflow.
    #[inline]
    pub const fn usable_capacity(&self) -> usize {
        if N == 0 {
            0
        } else {
            N - 1
        }
    }

    /// Number of elements currently available to pop.
    #[inline]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail)
    }

    /// True if the buffer contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True if the buffer has no free slots.
    #[inline]
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) == self.usable_capacity()
    }

    /// Attempt to push an element. Returns `Err(PushError::Full(value))` if full.
    ///
    /// # Safety / correctness
    /// Only one producer may call this method at a time (SPSC). This method uses
    /// `Ordering::Acquire` when loading `tail` and `Ordering::Release` when storing
    /// `head` to synchronize with the consumer.
    #[inline]
    pub fn push(&self, value: T) -> Result<(), PushError<T>> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= self.usable_capacity() {
            return Err(PushError::Full(value));
        }
        let idx = head & Self::MASK;
        // SAFETY: `idx` is < N by masking. We have a slot reserved (one-slot-empty),
        // and only the producer writes to this slot while `head == idx` and before
        // publishing `head+1`. The consumer will not read until it observes the new
        // `head` with Acquire. `MaybeUninit::write` does not drop the old value.
        unsafe {
            (*(*self.buffer.get()).as_mut_ptr().add(idx)).write(value);
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Attempt to pop an element. Returns `None` if empty.
    ///
    /// # Safety / correctness
    /// Only one consumer may call this method at a time (SPSC). Uses `Ordering::Acquire`
    /// when loading `head` and `Ordering::Release` when storing `tail`.
    #[inline]
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let idx = tail & Self::MASK;
        // SAFETY: Slot contains a value that was written by the producer and made
        // visible by `head` with Release/Acquire synchronization. We will advance
        // `tail` with Release after reading, so the producer can reuse this slot
        // only after observing the new `tail` with Acquire.
        let value = unsafe { (*(*self.buffer.get()).as_mut_ptr().add(idx)).assume_init_read() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// Clear all elements from the consumer side, dropping them.
    ///
    /// This is only safe when there is no concurrent producer/consumer activity.
    /// For SPSC it should be called when stopped or from the consumer thread.
    pub fn clear(&self) {
        while self.pop().is_some() {}
    }
}

impl<T, const N: usize> Default for InputRingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: `InputRingBuffer` can be shared across threads for SPSC handoff only if
// `T: Send`. The buffer uses atomics for synchronization and `UnsafeCell`-like
// access is properly synchronized via Acquire/Release. Only one producer and one
// consumer may access at a time.
unsafe impl<T: Send, const N: usize> Send for InputRingBuffer<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for InputRingBuffer<T, N> {}

impl<T, const N: usize> Drop for InputRingBuffer<T, N> {
    fn drop(&mut self) {
        while let Some(v) = self.pop() {
            drop(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn fifo_order() {
        let rb = InputRingBuffer::<RawInputEvent, 8>::new();
        let e1 = RawInputEvent {
            event_type: 1,
            code: 30,
            value: 1,
            timestamp: 100,
        };
        let e2 = RawInputEvent {
            event_type: 1,
            code: 30,
            value: 0,
            timestamp: 200,
        };
        assert!(rb.push(e1).is_ok());
        assert!(rb.push(e2).is_ok());
        assert_eq!(rb.pop().unwrap(), e1);
        assert_eq!(rb.pop().unwrap(), e2);
        assert!(rb.pop().is_none());
    }

    #[test]
    fn full_detection() {
        let rb = InputRingBuffer::<u32, 4>::new(); // usable 3
        assert_eq!(rb.usable_capacity(), 3);
        assert!(rb.push(1).is_ok());
        assert!(rb.push(2).is_ok());
        assert!(rb.push(3).is_ok());
        assert!(matches!(rb.push(4), Err(PushError::Full(4))));
        assert_eq!(rb.pop().unwrap(), 1);
        assert!(rb.push(4).is_ok());
    }

    #[test]
    fn wraparound() {
        let rb = InputRingBuffer::<u32, 8>::new();
        for i in 0..1000 {
            let v = i as u32;
            loop {
                match rb.push(v) {
                    Ok(()) => break,
                    Err(PushError::Full(_)) => {
                        let _ = rb.pop();
                    }
                }
            }
            if i % 3 == 0 {
                let _ = rb.pop();
            }
        }
        // drain remaining
        let mut count = 0;
        while rb.pop().is_some() {
            count += 1;
        }
        assert!(count < 8);
    }

    #[test]
    fn concurrent_spsc() {
        let rb = Arc::new(InputRingBuffer::<u64, 256>::new());
        let rb2 = rb.clone();
        let producer = thread::spawn(move || {
            for i in 0..10_000u64 {
                loop {
                    match rb2.push(i) {
                        Ok(()) => break,
                        Err(PushError::Full(v)) => {
                            // spin briefly by trying to pop nothing; just retry
                            let _ = v;
                            std::thread::yield_now();
                        }
                    }
                }
            }
        });
        let rb3 = rb.clone();
        let mut received = Vec::with_capacity(10_000);
        let consumer = thread::spawn(move || {
            let mut missing = 0;
            while received.len() < 10_000 {
                match rb3.pop() {
                    Some(v) => received.push(v),
                    None => {
                        missing += 1;
                        if missing > 10_000_000 {
                            break;
                        }
                        std::thread::yield_now();
                    }
                }
            }
            received
        });
        producer.join().unwrap();
        let received = consumer.join().unwrap();
        assert_eq!(received.len(), 10_000);
        for (i, v) in received.iter().enumerate() {
            assert_eq!(*v as usize, i);
        }
    }
}
