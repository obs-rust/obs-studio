//! Safe core for `libobs/util/deque.h`, the header-inline byte ring buffer.
//!
//! [`Deque`] reproduces the C functions step for step: the same four size
//! fields ([`DequeState`]), the same growth rule (the first push allocates
//! exactly its size, then the capacity doubles, or becomes the new size when
//! that is larger), the same wrap points and the same move of the wrapped
//! tail when the buffer grows. It is generic over its byte buffer
//! ([`DequeBuf`]) so one implementation serves both the safe `Vec` buffer
//! ([`VecBuf`]) and a C-owned `struct deque` whose memory is `bmalloc`'d
//! ([`crate::ffi::deque`]), which C's `deque_free` can then free.
//!
//! Characterized, not endorsed (as in C): a full unwrapped deque, or one
//! popped at the back by exactly `end_pos` bytes, has `end_pos == capacity`,
//! not 0.

/// The size fields of `struct deque`, in bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DequeState {
    pub size: usize,
    pub start_pos: usize,
    pub end_pos: usize,
    pub capacity: usize,
}

/// The byte buffer behind a [`Deque`].
///
/// [`bytes`](Self::bytes) is the whole allocation (the deque's `capacity`
/// bytes), live or not.
pub trait DequeBuf {
    /// `brealloc(data, len)`: allocates, or resizes to `len` bytes keeping
    /// the first `min(old, len)` bytes. A deque only grows its buffer.
    fn realloc(&mut self, len: usize);
    /// `bfree(data)`: releases the buffer; nothing to do when there is none.
    fn free(&mut self);
    fn bytes(&self) -> &[u8];
    fn bytes_mut(&mut self) -> &mut [u8];
}

/// A `Vec` buffer. Bytes a reallocation adds are zero (C leaves them
/// uninitialized; no deque function reads them before writing them).
#[derive(Debug, Clone, Default)]
pub struct VecBuf(Vec<u8>);

impl DequeBuf for VecBuf {
    fn realloc(&mut self, len: usize) {
        self.0.resize(len, 0);
    }

    fn free(&mut self) {
        self.0 = Vec::new();
    }

    fn bytes(&self) -> &[u8] {
        &self.0
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

/// Double-ended byte queue with the layout and behaviour of C `struct deque`.
#[derive(Debug, Clone, Default)]
pub struct Deque<B: DequeBuf = VecBuf> {
    st: DequeState,
    buf: B,
}

impl Deque<VecBuf> {
    /// `deque_init`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<B: DequeBuf> Deque<B> {
    /// A deque over existing state and buffer. `buf.bytes()` must be at
    /// least `state.capacity` bytes and the state one a deque can reach.
    pub fn from_parts(state: DequeState, buf: B) -> Self {
        Self { st: state, buf }
    }

    pub fn into_parts(self) -> (DequeState, B) {
        (self.st, self.buf)
    }

    pub fn state(&self) -> DequeState {
        todo!()
    }

    /// Live bytes.
    pub fn size(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }

    pub fn start_pos(&self) -> usize {
        todo!()
    }

    pub fn end_pos(&self) -> usize {
        todo!()
    }

    pub fn capacity(&self) -> usize {
        todo!()
    }

    /// The whole buffer, as `dq->data[0..capacity]` (empty before the first
    /// allocation and after [`free`](Self::free)).
    pub fn raw(&self) -> &[u8] {
        todo!()
    }

    pub fn buf(&self) -> &B {
        todo!()
    }

    /// `deque_free`: releases the buffer and zeroes the state.
    pub fn free(&mut self) {
        todo!()
    }

    /// `deque_reserve`: never shrinks.
    pub fn reserve(&mut self, capacity: usize) {
        todo!()
    }

    /// `deque_upsize`: grows the size to `size`, zero-filling at the back.
    pub fn upsize(&mut self, size: usize) {
        todo!()
    }

    /// `deque_place`: overwrites `data` at `position` bytes from the front,
    /// upsizing first when it ends past the size.
    pub fn place(&mut self, position: usize, data: &[u8]) {
        todo!()
    }

    /// `deque_push_back`.
    pub fn push_back(&mut self, data: &[u8]) {
        todo!()
    }

    /// `deque_push_front`.
    pub fn push_front(&mut self, data: &[u8]) {
        todo!()
    }

    /// `deque_push_back_zero`.
    pub fn push_back_zero(&mut self, size: usize) {
        todo!()
    }

    /// `deque_push_front_zero`.
    pub fn push_front_zero(&mut self, size: usize) {
        todo!()
    }

    /// `deque_peek_front`: copies the first `out.len()` bytes. Panics if
    /// that is more than the size (C asserts).
    pub fn peek_front(&self, out: &mut [u8]) {
        todo!()
    }

    /// `deque_peek_back`: copies the last `out.len()` bytes. Panics if that
    /// is more than the size (C asserts).
    ///
    /// The `end_pos == 0` fallback to the capacity is kept from C, though no
    /// sequence of deque calls leaves `end_pos` at 0 with live bytes; from
    /// such a hand-made state C reads before the buffer and this panics.
    pub fn peek_back(&self, out: &mut [u8]) {
        todo!()
    }

    /// `deque_pop_front`: [`peek_front`](Self::peek_front), then removes
    /// `out.len()` bytes.
    pub fn pop_front(&mut self, out: &mut [u8]) {
        todo!()
    }

    /// `deque_pop_back`: [`peek_back`](Self::peek_back), then removes
    /// `out.len()` bytes.
    pub fn pop_back(&mut self, out: &mut [u8]) {
        todo!()
    }

    /// `deque_pop_front` with a null destination: removes `size` bytes.
    pub fn discard_front(&mut self, size: usize) {
        todo!()
    }

    /// `deque_pop_back` with a null destination: removes `size` bytes.
    pub fn discard_back(&mut self, size: usize) {
        todo!()
    }

    /// `deque_data`: the buffer offset of byte `idx` from the front, or
    /// `None` (C: `NULL`) when `idx >= size`.
    pub fn data_offset(&self, idx: usize) -> Option<usize> {
        todo!()
    }
}
