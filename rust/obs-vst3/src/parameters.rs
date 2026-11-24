#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub offset: i32,
    pub value: f64,
}

#[derive(Debug, Default)]
pub struct Queue {
    points: Vec<Point>,
}

impl Queue {
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn insert(&mut self, point: Point) -> usize {
        match self
            .points
            .binary_search_by_key(&point.offset, |current| current.offset)
        {
            Ok(index) => {
                self.points[index] = point;
                index
            }
            Err(index) => {
                self.points.insert(index, point);
                index
            }
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub id: u32,
    pub point: Point,
}

#[derive(Default)]
struct Slot {
    id: AtomicU32,
    offset: AtomicI32,
    value: AtomicU64,
}

/// A fixed-capacity handoff between the host's one UI thread and one audio thread.
/// Like the SDK's ParameterChangeTransfer, one buffer entry separates full from empty.
pub struct Transfer {
    slots: Box<[Slot]>,
    read: AtomicUsize,
    write: AtomicUsize,
}

impl Transfer {
    pub fn new(parameter_count: usize) -> Self {
        let count = if parameter_count == 0 {
            256
        } else {
            parameter_count
        };
        Self {
            slots: (0..count.saturating_mul(2))
                .map(|_| Slot::default())
                .collect(),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, change: Change) -> bool {
        let write = self.write.load(Ordering::Relaxed);
        let next = (write + 1) % self.slots.len();
        if next == self.read.load(Ordering::Acquire) {
            return false;
        }
        let slot = &self.slots[write];
        slot.id.store(change.id, Ordering::Relaxed);
        slot.offset.store(change.point.offset, Ordering::Relaxed);
        slot.value
            .store(change.point.value.to_bits(), Ordering::Relaxed);
        self.write.store(next, Ordering::Release);
        true
    }

    pub fn pop(&self) -> Option<Change> {
        let read = self.read.load(Ordering::Relaxed);
        if read == self.write.load(Ordering::Acquire) {
            return None;
        }
        let slot = &self.slots[read];
        let change = Change {
            id: slot.id.load(Ordering::Relaxed),
            point: Point {
                offset: slot.offset.load(Ordering::Relaxed),
                value: f64::from_bits(slot.value.load(Ordering::Relaxed)),
            },
        };
        self.read
            .store((read + 1) % self.slots.len(), Ordering::Release);
        Some(change)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Restart {
    pub latency_changed: bool,
    pub values_changed: bool,
}

impl Restart {
    pub fn decode(flags: i32) -> Option<Self> {
        let latency_changed = flags & (1 << 3) != 0;
        let values_changed = flags & (1 << 2) != 0;
        (latency_changed || values_changed).then_some(Self {
            latency_changed,
            values_changed,
        })
    }
}

use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};
