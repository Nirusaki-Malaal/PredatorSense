// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded FIFO for concurrent WMI producers and the process-context consumer.
#![forbid(unsafe_code)]
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
struct Slot {
    sequence: AtomicUsize,
    value: AtomicU64,
}
pub(crate) struct EventQueue<const N: usize> {
    slots: [Slot; N],
    enqueue: AtomicUsize,
    dequeue: AtomicUsize,
}
impl<const N: usize> EventQueue<N> {
    pub(crate) const fn new() -> Self {
        let mut slots = [const {
            Slot {
                sequence: AtomicUsize::new(0),
                value: AtomicU64::new(0),
            }
        }; N];
        let mut index = 0;
        while index < N {
            slots[index].sequence = AtomicUsize::new(index);
            index += 1;
        }
        Self {
            slots,
            enqueue: AtomicUsize::new(0),
            dequeue: AtomicUsize::new(0),
        }
    }
    pub(crate) fn push(&self, value: u64) -> bool {
        let mut position = self.enqueue.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[position % N];
            let sequence = slot.sequence.load(Ordering::Acquire);
            let difference = sequence.wrapping_sub(position) as isize;
            if difference == 0 {
                match self.enqueue.compare_exchange_weak(
                    position,
                    position.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        slot.value.store(value, Ordering::Relaxed);
                        slot.sequence
                            .store(position.wrapping_add(1), Ordering::Release);
                        return true;
                    }
                    Err(current) => position = current,
                }
            } else if difference < 0 {
                return false;
            } else {
                position = self.enqueue.load(Ordering::Relaxed);
            }
        }
    }
    pub(crate) fn pop(&self) -> Option<u64> {
        let mut position = self.dequeue.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[position % N];
            let sequence = slot.sequence.load(Ordering::Acquire);
            let difference = sequence.wrapping_sub(position.wrapping_add(1)) as isize;
            if difference == 0 {
                match self.dequeue.compare_exchange_weak(
                    position,
                    position.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let value = slot.value.load(Ordering::Relaxed);
                        slot.sequence
                            .store(position.wrapping_add(N), Ordering::Release);
                        return Some(value);
                    }
                    Err(current) => position = current,
                }
            } else if difference < 0 {
                return None;
            } else {
                position = self.dequeue.load(Ordering::Relaxed);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraparound_fifo() {
        let queue = EventQueue::<4>::new();
        for base in 0..100 {
            for value in base * 4..base * 4 + 4 {
                assert!(queue.push(value));
            }
            assert!(!queue.push(999));
            for value in base * 4..base * 4 + 4 {
                assert_eq!(queue.pop(), Some(value));
            }
            assert_eq!(queue.pop(), None);
        }
    }
    #[test]
    fn concurrent_producers() {
        let queue = std::sync::Arc::new(EventQueue::<16>::new());
        let threads: Vec<_> = (0..4)
            .map(|producer| {
                let queue = queue.clone();
                std::thread::spawn(move || {
                    for index in 0..250 {
                        let value = producer * 250 + index;
                        while !queue.push(value) {
                            std::thread::yield_now();
                        }
                    }
                })
            })
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        while seen.len() < 1000 {
            if let Some(value) = queue.pop() {
                assert!(seen.insert(value));
            } else {
                std::thread::yield_now();
            }
        }
        for thread in threads {
            thread.join().unwrap();
        }
    }
}
