//! Safe core for `libobs/util/darray.h`.
//!
//! [`DArray`] models the C growth semantics (an explicit C-model `capacity`
//! that follows `darray_ensure_capacity`) on top of a `Vec`. The raw
//! `struct darray` mirror lives in [`crate::ffi::darray`].

/// `DARRAY_INVALID` from `darray.h`.
pub const DARRAY_INVALID: usize = usize::MAX;

/// New capacity chosen by `darray_ensure_capacity`. Only meaningful when
/// `new_size > capacity`.
pub fn grow_capacity(capacity: usize, new_size: usize) -> usize {
    let new_cap = if capacity == 0 {
        new_size
    } else {
        capacity.wrapping_mul(2)
    };
    new_cap.max(new_size)
}

/// Dynamic array with the capacity behaviour of C `struct darray`.
#[derive(Debug, Clone, Default)]
pub struct DArray<T: Copy + Default> {
    items: Vec<T>,
    capacity: usize,
}

impl<T: Copy + Default> DArray<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            capacity: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The C-model capacity (not the `Vec` capacity).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items
    }

    /// `darray_reserve`: never shrinks.
    pub fn reserve(&mut self, cap: usize) {
        if cap == 0 || cap <= self.capacity {
            return;
        }
        self.capacity = cap;
    }

    /// `darray_ensure_capacity`.
    pub fn ensure_capacity(&mut self, new_size: usize) {
        if new_size <= self.capacity {
            return;
        }
        self.capacity = grow_capacity(self.capacity, new_size);
    }

    /// `darray_resize`: new items are `T::default()`.
    pub fn resize(&mut self, size: usize) {
        if size == self.items.len() {
            return;
        }
        if size == 0 {
            self.items.clear();
            return;
        }
        self.ensure_capacity(size);
        self.items.resize(size, T::default());
    }

    /// `darray_push_back`; returns the new item's index.
    pub fn push_back(&mut self, item: T) -> usize {
        let idx = self.items.len();
        self.ensure_capacity(idx + 1);
        self.items.push(item);
        idx
    }

    /// `darray_push_back_array`; returns the old length.
    pub fn push_back_array(&mut self, array: &[T]) -> usize {
        let old_len = self.items.len();
        if array.is_empty() {
            return old_len;
        }
        self.resize(old_len + array.len());
        self.items[old_len..].copy_from_slice(array);
        old_len
    }

    /// `darray_clear`: keeps capacity.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// `darray_free`.
    pub fn free(&mut self) {
        self.items = Vec::new();
        self.capacity = 0;
    }

    /// `darray_erase`; out-of-range indices are ignored.
    pub fn erase(&mut self, idx: usize) {
        if idx < self.items.len() {
            self.items.remove(idx);
        }
    }

    /// `darray_pop_back`; no-op when empty.
    pub fn pop_back(&mut self) {
        self.items.pop();
    }

    /// `darray_push_back_new` (and the GCC/Clang `da_push_back_new` macro,
    /// which does the same): appends a zeroed item (`T::default()`) and
    /// returns it.
    pub fn push_back_new(&mut self) -> &mut T {
        let idx = self.items.len();
        self.ensure_capacity(idx + 1);
        self.items.push(T::default());
        &mut self.items[idx]
    }

    /// `darray_insert`: `idx == len` appends.
    ///
    /// # Panics
    ///
    /// When `idx > len`, where C asserts.
    pub fn insert(&mut self, idx: usize, item: T) {
        assert!(idx <= self.items.len(), "darray_insert: idx > num");
        if idx == self.items.len() {
            self.push_back(item);
            return;
        }
        self.ensure_capacity(self.items.len() + 1);
        self.items.insert(idx, item);
    }

    /// `darray_erase_range`: removes `start..end`. Never shrinks the
    /// capacity.
    ///
    /// # Panics
    ///
    /// Unless `start < end <= len`, where C asserts.
    pub fn erase_range(&mut self, start: usize, end: usize) {
        let len = self.items.len();
        assert!(start <= len, "darray_erase_range: start > num");
        assert!(end <= len, "darray_erase_range: end > num");
        assert!(end > start, "darray_erase_range: end <= start");
        self.items.drain(start..end);
    }

    /// `darray_pop_front`; no-op when empty (the C body checks `num` after
    /// its assert).
    pub fn pop_front(&mut self) {
        if !self.items.is_empty() {
            self.items.remove(0);
        }
    }
}

impl<T: Copy + Default + PartialEq> DArray<T> {
    /// `darray_find`; `start` must be `<= len`.
    pub fn find(&self, item: &T, start: usize) -> usize {
        assert!(start <= self.items.len());
        self.items[start..]
            .iter()
            .position(|x| x == item)
            .map_or(DARRAY_INVALID, |i| start + i)
    }
}
