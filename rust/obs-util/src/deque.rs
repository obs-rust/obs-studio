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

/// The source of a push: caller bytes, or zeroes (the `_zero` variants).
#[derive(Clone, Copy)]
enum Src<'a> {
    Bytes(&'a [u8]),
    Zero,
}

impl Src<'_> {
    /// Fills `dst` from the source, starting at byte `from` of it.
    fn copy_to(self, dst: &mut [u8], from: usize) {
        match self {
            Src::Bytes(b) => dst.copy_from_slice(&b[from..from + dst.len()]),
            Src::Zero => dst.fill(0),
        }
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
        self.st
    }

    /// Live bytes.
    pub fn size(&self) -> usize {
        self.st.size
    }

    pub fn is_empty(&self) -> bool {
        self.st.size == 0
    }

    pub fn start_pos(&self) -> usize {
        self.st.start_pos
    }

    pub fn end_pos(&self) -> usize {
        self.st.end_pos
    }

    pub fn capacity(&self) -> usize {
        self.st.capacity
    }

    /// The whole buffer, as `dq->data[0..capacity]` (empty before the first
    /// allocation and after [`free`](Self::free)).
    pub fn raw(&self) -> &[u8] {
        self.buf.bytes()
    }

    pub fn buf(&self) -> &B {
        &self.buf
    }

    /// `deque_free`: releases the buffer and zeroes the state.
    pub fn free(&mut self) {
        self.buf.free();
        self.st = DequeState::default();
    }

    /// `deque_reorder_data`: after the buffer grew to `new_capacity`, moves
    /// wrapped bytes from `start_pos` to the old end up to the new end.
    fn reorder_data(&mut self, new_capacity: usize) {
        let st = &mut self.st;
        if st.size == 0 || st.start_pos == 0 || st.end_pos > st.start_pos {
            return;
        }
        let difference = new_capacity - st.capacity;
        self.buf
            .bytes_mut()
            .copy_within(st.start_pos..st.capacity, st.start_pos + difference);
        st.start_pos += difference;
    }

    /// `deque_ensure_capacity`: grows for the already increased `size`.
    fn ensure_capacity(&mut self) {
        if self.st.size <= self.st.capacity {
            return;
        }
        let new_capacity = self.st.capacity.wrapping_mul(2).max(self.st.size);
        self.buf.realloc(new_capacity);
        self.reorder_data(new_capacity);
        self.st.capacity = new_capacity;
    }

    /// `deque_reserve`: never shrinks.
    pub fn reserve(&mut self, capacity: usize) {
        if capacity <= self.st.capacity {
            return;
        }
        self.buf.realloc(capacity);
        self.reorder_data(capacity);
        self.st.capacity = capacity;
    }

    /// `deque_upsize`: grows the size to `size`, zero-filling at the back.
    pub fn upsize(&mut self, size: usize) {
        if size <= self.st.size {
            return;
        }
        // The C body is `deque_push_back_zero` of the difference.
        self.push_back_src(Src::Zero, size - self.st.size);
    }

    /// `deque_place`: overwrites `data` at `position` bytes from the front,
    /// upsizing first when it ends past the size.
    pub fn place(&mut self, position: usize, data: &[u8]) {
        let size = data.len();
        let end_point = position + size;
        if end_point > self.st.size {
            self.upsize(end_point);
        }
        let mut position = position + self.st.start_pos;
        let capacity = self.st.capacity;
        if position >= capacity {
            position -= capacity;
        }
        let buf = self.buf.bytes_mut();
        let data_end_pos = position + size;
        if data_end_pos > capacity {
            let back_size = data_end_pos - capacity;
            let loop_size = size - back_size;
            buf[position..position + loop_size].copy_from_slice(&data[..loop_size]);
            buf[..back_size].copy_from_slice(&data[loop_size..]);
        } else {
            buf[position..data_end_pos].copy_from_slice(data);
        }
    }

    fn push_back_src(&mut self, src: Src<'_>, size: usize) {
        let new_end_pos = self.st.end_pos + size;
        self.st.size += size;
        self.ensure_capacity();

        let DequeState {
            end_pos, capacity, ..
        } = self.st;
        let buf = self.buf.bytes_mut();
        if new_end_pos > capacity {
            let back_size = capacity - end_pos;
            let loop_size = size - back_size;
            if back_size != 0 {
                src.copy_to(&mut buf[end_pos..capacity], 0);
            }
            src.copy_to(&mut buf[..loop_size], back_size);
            self.st.end_pos = new_end_pos - capacity;
        } else {
            src.copy_to(&mut buf[end_pos..new_end_pos], 0);
            self.st.end_pos = new_end_pos;
        }
    }

    fn push_front_src(&mut self, src: Src<'_>, size: usize) {
        self.st.size += size;
        self.ensure_capacity();

        let capacity = self.st.capacity;
        let start_pos = self.st.start_pos;
        let buf = self.buf.bytes_mut();
        if self.st.size == size {
            src.copy_to(&mut buf[..size], 0);
            self.st.start_pos = 0;
            self.st.end_pos = size;
        } else if start_pos < size {
            let back_size = size - start_pos;
            if start_pos != 0 {
                src.copy_to(&mut buf[..start_pos], back_size);
            }
            let new_start = capacity - back_size;
            src.copy_to(&mut buf[new_start..capacity], 0);
            self.st.start_pos = new_start;
        } else {
            let new_start = start_pos - size;
            src.copy_to(&mut buf[new_start..start_pos], 0);
            self.st.start_pos = new_start;
        }
    }

    /// `deque_push_back`.
    pub fn push_back(&mut self, data: &[u8]) {
        self.push_back_src(Src::Bytes(data), data.len());
    }

    /// `deque_push_front`.
    pub fn push_front(&mut self, data: &[u8]) {
        self.push_front_src(Src::Bytes(data), data.len());
    }

    /// `deque_push_back_zero`.
    pub fn push_back_zero(&mut self, size: usize) {
        self.push_back_src(Src::Zero, size);
    }

    /// `deque_push_front_zero`.
    pub fn push_front_zero(&mut self, size: usize) {
        self.push_front_src(Src::Zero, size);
    }

    /// `deque_peek_front`: copies the first `out.len()` bytes. Panics if
    /// that is more than the size (C asserts).
    pub fn peek_front(&self, out: &mut [u8]) {
        let size = out.len();
        assert!(size <= self.st.size, "deque_peek_front past the size");
        let DequeState {
            start_pos,
            capacity,
            ..
        } = self.st;
        let buf = self.buf.bytes();
        let start_size = capacity - start_pos;
        if start_size < size {
            out[..start_size].copy_from_slice(&buf[start_pos..capacity]);
            out[start_size..].copy_from_slice(&buf[..size - start_size]);
        } else {
            out.copy_from_slice(&buf[start_pos..start_pos + size]);
        }
    }

    /// `deque_peek_back`: copies the last `out.len()` bytes. Panics if that
    /// is more than the size (C asserts).
    ///
    /// The `end_pos == 0` fallback to the capacity is kept from C, though no
    /// sequence of deque calls leaves `end_pos` at 0 with live bytes; from
    /// such a hand-made state C reads before the buffer and this panics.
    pub fn peek_back(&self, out: &mut [u8]) {
        let size = out.len();
        assert!(size <= self.st.size, "deque_peek_back past the size");
        let DequeState {
            end_pos, capacity, ..
        } = self.st;
        let buf = self.buf.bytes();
        let back_size = if end_pos != 0 { end_pos } else { capacity };
        if back_size < size {
            let front_size = size - back_size;
            let new_end_pos = capacity - front_size;
            out[size - back_size..].copy_from_slice(&buf[..back_size]);
            out[..front_size].copy_from_slice(&buf[new_end_pos..capacity]);
        } else {
            out.copy_from_slice(&buf[end_pos - size..end_pos]);
        }
    }

    /// `deque_pop_front`: [`peek_front`](Self::peek_front), then removes
    /// `out.len()` bytes.
    pub fn pop_front(&mut self, out: &mut [u8]) {
        self.peek_front(out);
        self.consume_front(out.len());
    }

    /// `deque_pop_back`: [`peek_back`](Self::peek_back), then removes
    /// `out.len()` bytes.
    pub fn pop_back(&mut self, out: &mut [u8]) {
        self.peek_back(out);
        self.consume_back(out.len());
    }

    /// `deque_pop_front` with a null destination: removes `size` bytes.
    pub fn discard_front(&mut self, size: usize) {
        assert!(size <= self.st.size, "deque_pop_front past the size");
        self.consume_front(size);
    }

    /// `deque_pop_back` with a null destination: removes `size` bytes.
    pub fn discard_back(&mut self, size: usize) {
        assert!(size <= self.st.size, "deque_pop_back past the size");
        self.consume_back(size);
    }

    fn consume_front(&mut self, size: usize) {
        let st = &mut self.st;
        st.size -= size;
        if st.size == 0 {
            st.start_pos = 0;
            st.end_pos = 0;
            return;
        }
        st.start_pos += size;
        if st.start_pos >= st.capacity {
            st.start_pos -= st.capacity;
        }
    }

    fn consume_back(&mut self, size: usize) {
        let st = &mut self.st;
        st.size -= size;
        if st.size == 0 {
            st.start_pos = 0;
            st.end_pos = 0;
            return;
        }
        if st.end_pos <= size {
            st.end_pos = st.capacity - (size - st.end_pos);
        } else {
            st.end_pos -= size;
        }
    }

    /// `deque_data`: the buffer offset of byte `idx` from the front, or
    /// `None` (C: `NULL`) when `idx >= size`.
    pub fn data_offset(&self, idx: usize) -> Option<usize> {
        if idx >= self.st.size {
            return None;
        }
        let mut offset = self.st.start_pos + idx;
        if offset >= self.st.capacity {
            offset -= self.st.capacity;
        }
        Some(offset)
    }
}
