//! Shared primitives for input drivers.
//!
//! The goal is to make input devices declarative and self-describing so new
//! drivers can be composed from the same building blocks without having to
//! reinvent interrupt-time buffering or event plumbing.

use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::instructions::interrupts;

/// Simple lock-free ring buffer for interrupt handlers.
///
/// Writers only fail when the buffer is full; readers pop in FIFO order.
/// Designed to be easy to reuse across devices so LLMS can plug it into
/// new drivers with minimal ceremony.
pub struct InputBuffer<T: Copy + Default, const N: usize> {
    buf: Mutex<[T; N]>,
    head: AtomicUsize,
    tail: AtomicUsize,
    default: T,
}

impl<T: Copy + Default, const N: usize> InputBuffer<T, N> {
    pub const fn new(default: T) -> Self {
        Self {
            buf: Mutex::new([default; N]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            default,
        }
    }

    /// Push a value; returns `Err(value)` if the buffer is full.
    pub fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let next = (head + 1) % N;

        if next == tail {
            return Err(value);
        }

        if let Some(mut buf) = self.buf.try_lock() {
            buf[head] = value;
            self.head.store(next, Ordering::Release);
            Ok(())
        } else {
            Err(value)
        }
    }

    pub fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);

        if head == tail {
            return None;
        }

        let mut buf = self.buf.lock();
        let value = buf[tail];
        buf[tail] = self.default;
        self.tail.store((tail + 1) % N, Ordering::Release);
        Some(value)
    }

    pub fn clear(&self) {
        let mut buf = self.buf.lock();
        buf.fill(self.default);
        self.head.store(0, Ordering::Relaxed);
        self.tail.store(0, Ordering::Relaxed);
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Relaxed)
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        if head >= tail {
            head - tail
        } else {
            N - tail + head
        }
    }

    /// Return the raw head/tail offsets. Useful for diagnostics and graph events.
    pub fn positions(&self) -> (usize, usize) {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        (head, tail)
    }
}
