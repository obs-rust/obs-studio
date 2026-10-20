//! Tier 1: safe-core tests. Each test named after a case mirrors that case of
//! `test/cmocka/test_deque.c` 1:1 (`test_layout` is the Tier 2 layout test in
//! `deque_layout.rs`); the rest are new edge cases.
//!
//! `bnum_allocs()` checks are ported through [`CountingBuf`], a buffer that
//! counts like bmem does (`brealloc(NULL, n)` is an allocation, `brealloc` of
//! a block is not, `bfree` of a block releases one), so they need no
//! process-wide counter. The bmem path itself is checked in `deque_parity.rs`.

use std::cell::Cell;
use std::rc::Rc;

use obs_c_oracle as _;
use obs_util::deque::{Deque, DequeBuf, DequeState, VecBuf};

/// A `Vec` buffer that counts live allocations into a shared counter.
#[derive(Debug, Default)]
struct CountingBuf {
    bytes: Vec<u8>,
    allocated: bool,
    allocs: Rc<Cell<i64>>,
}

impl DequeBuf for CountingBuf {
    fn realloc(&mut self, len: usize) {
        if !self.allocated {
            self.allocated = true;
            self.allocs.set(self.allocs.get() + 1);
        }
        self.bytes.resize(len, 0);
    }

    fn free(&mut self) {
        if self.allocated {
            self.allocated = false;
            self.allocs.set(self.allocs.get() - 1);
        }
        self.bytes = Vec::new();
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

/// A counting deque and its counter (C: `bnum_allocs()`).
fn counted() -> (Deque<CountingBuf>, Rc<Cell<i64>>) {
    let allocs = Rc::new(Cell::new(0));
    let buf = CountingBuf {
        allocs: Rc::clone(&allocs),
        ..CountingBuf::default()
    };
    (Deque::from_parts(DequeState::default(), buf), allocs)
}

#[track_caller]
fn assert_state<B: DequeBuf>(
    dq: &Deque<B>,
    size: usize,
    start_pos: usize,
    end_pos: usize,
    capacity: usize,
) {
    assert_eq!(
        dq.state(),
        DequeState {
            size,
            start_pos,
            end_pos,
            capacity
        }
    );
}

#[track_caller]
fn assert_front<B: DequeBuf>(dq: &Deque<B>, expected: &[u8]) {
    let mut buf = vec![0u8; expected.len()];
    dq.peek_front(&mut buf);
    assert_eq!(buf, expected);
}

#[track_caller]
fn assert_raw<B: DequeBuf>(dq: &Deque<B>, offset: usize, expected: &[u8]) {
    assert_eq!(&dq.raw()[offset..offset + expected.len()], expected);
}

/// A deque with capacity 8 holding "abcdef" from position 0.
fn make_abcdef_in_8<B: DequeBuf>(dq: &mut Deque<B>) {
    dq.free();
    dq.reserve(8);
    dq.push_back(b"abcdef");
    assert_state(dq, 6, 0, 6, 8);
}

/// Full and wrapped: raw "XYZdefgh", start_pos 3, end_pos 3, capacity 8.
fn make_full_wrapped<B: DequeBuf>(dq: &mut Deque<B>) {
    dq.free();
    dq.push_back(b"abcdefgh");
    dq.discard_front(3);
    dq.push_back(b"XYZ");
    assert_state(dq, 8, 3, 3, 8);
    assert_raw(dq, 0, b"XYZdefgh");
}

#[test]
fn init_zeroes() {
    let dq = Deque::new();
    assert!(dq.raw().is_empty());
    assert_state(&dq, 0, 0, 0, 0);
    assert_eq!(dq.data_offset(0), None);
}

#[test]
fn growth_doubles_or_takes_size() {
    let (mut dq, allocs) = counted();

    // The first push allocates exactly its size.
    dq.push_back(b"abcd");
    assert_state(&dq, 4, 0, 4, 4);
    assert_eq!(allocs.get(), 1);

    // Then the capacity doubles...
    dq.push_back(b"e");
    assert_state(&dq, 5, 0, 5, 8);

    dq.push_back(b"fghijklmno");
    assert_state(&dq, 15, 0, 15, 16);

    // ...unless the new size is larger than double, then it is the size.
    dq.push_back(&[b'z'; 20]);
    assert_state(&dq, 35, 0, 35, 35);

    // A full, unwrapped deque has end_pos == capacity, not 0.
    assert_eq!(dq.end_pos(), dq.capacity());

    // brealloc of an existing block is not a new allocation.
    assert_eq!(allocs.get(), 1);
    assert_front(&dq, b"abcdefghijklmno");

    dq.free();
    assert_eq!(allocs.get(), 0);
}

#[test]
fn growth_takes_size_from_small() {
    let mut dq = Deque::new();
    dq.push_back(b"abc");
    assert_state(&dq, 3, 0, 3, 3);
    dq.push_back(b"defghij");
    assert_state(&dq, 10, 0, 10, 10);
    assert_front(&dq, b"abcdefghij");
    dq.free();
}

#[test]
fn pop_front() {
    let (mut dq, allocs) = counted();
    let mut buf = [0u8; 3];

    dq.push_back(b"abcdefgh");

    dq.pop_front(&mut buf);
    assert_eq!(&buf, b"abc");
    assert_state(&dq, 5, 3, 8, 8);
    assert_front(&dq, b"defgh");

    // Popping to empty resets both positions and keeps the buffer.
    dq.discard_front(5);
    assert_state(&dq, 0, 0, 0, 8);
    assert_eq!(dq.raw().len(), 8);
    assert_eq!(allocs.get(), 1);

    // The next push starts at position 0 again, without reallocating.
    dq.push_back(b"ij");
    assert_state(&dq, 2, 0, 2, 8);
    assert_raw(&dq, 0, b"ij");
    assert_eq!(allocs.get(), 1);

    dq.free();
    assert_eq!(allocs.get(), 0);
}

#[test]
fn push_back_wraps_at_end() {
    let mut dq = Deque::new();

    // end_pos == capacity: the whole push goes to the start.
    make_full_wrapped(&mut dq);
    assert_front(&dq, b"defghXYZ");
    dq.free();

    // end_pos < capacity: the push is split across the end.
    make_abcdef_in_8(&mut dq);
    dq.discard_front(4);
    assert_state(&dq, 2, 4, 6, 8);
    dq.push_back(b"UVWX");
    assert_state(&dq, 6, 4, 2, 8);
    assert_raw(&dq, 0, b"WXcdefUV");
    assert_front(&dq, b"efUVWX");
    dq.free();
}

#[test]
fn growth_moves_wrapped_tail() {
    let mut dq = Deque::new();
    make_full_wrapped(&mut dq);

    // The bytes from start_pos to the old end move to the new end; the
    // wrapped head stays at 0 and the push lands after it.
    dq.push_back(b"1");
    assert_state(&dq, 9, 11, 4, 16);
    assert_raw(&dq, 0, b"XYZ1");
    assert_raw(&dq, 11, b"defgh");
    assert_front(&dq, b"defghXYZ1");
    dq.free();
}

#[test]
fn growth_unwrapped_with_offset_does_not_move() {
    let mut dq = Deque::new();
    dq.push_back(b"abcdefgh");
    dq.discard_front(2);
    assert_state(&dq, 6, 2, 8, 8);

    // Without growth this push would wrap; after growth it does not.
    dq.push_back(b"ijk");
    assert_state(&dq, 9, 2, 11, 16);
    assert_raw(&dq, 2, b"cdefghijk");
    assert_front(&dq, b"cdefghijk");
    dq.free();
}

#[test]
fn reserve() {
    let (mut dq, allocs) = counted();

    dq.reserve(0);
    assert!(dq.raw().is_empty());
    assert_state(&dq, 0, 0, 0, 0);

    dq.reserve(16);
    assert_eq!(dq.raw().len(), 16);
    assert_state(&dq, 0, 0, 0, 16);
    assert_eq!(allocs.get(), 1);

    // Never shrinks.
    dq.reserve(8);
    assert_state(&dq, 0, 0, 0, 16);
    dq.free();
    assert_eq!(allocs.get(), 0);

    // Reserving on wrapped data moves the tail by the difference.
    make_full_wrapped(&mut dq);
    dq.reserve(12);
    assert_state(&dq, 8, 7, 3, 12);
    assert_raw(&dq, 0, b"XYZ");
    assert_raw(&dq, 7, b"defgh");
    assert_front(&dq, b"defghXYZ");

    // The gap fills up to exactly the capacity, with no growth.
    dq.push_back(b"PQRS");
    assert_state(&dq, 12, 7, 7, 12);
    assert_front(&dq, b"defghXYZPQRS");
    dq.free();
}

#[test]
fn push_front() {
    let mut dq = Deque::new();

    // Into an empty deque: at position 0.
    dq.push_front(b"abc");
    assert_state(&dq, 3, 0, 3, 3);
    assert_raw(&dq, 0, b"abc");

    // start_pos 0: wraps to the end of the (grown) buffer.
    dq.push_front(b"Z");
    assert_state(&dq, 4, 5, 3, 6);
    assert_raw(&dq, 5, b"Z");
    assert_front(&dq, b"Zabc");

    // start_pos >= size: just before start_pos.
    dq.push_front(b"Y");
    assert_state(&dq, 5, 4, 3, 6);
    assert_front(&dq, b"YZabc");
    dq.free();

    // 0 < start_pos < size: split, the tail of the push at 0.
    make_abcdef_in_8(&mut dq);
    dq.discard_front(2);
    dq.push_front(b"WXYZ");
    assert_state(&dq, 8, 6, 6, 8);
    assert_raw(&dq, 0, b"YZcdefWX");
    assert_front(&dq, b"WXYZcdef");

    // Into a full wrapped deque: growth moves the tail first.
    dq.push_front(b"V");
    assert_state(&dq, 9, 13, 6, 16);
    assert_front(&dq, b"VWXYZcdef");
    dq.free();
}

#[test]
fn push_zero() {
    let mut dq = Deque::new();

    make_abcdef_in_8(&mut dq);
    dq.discard_front(4);
    assert_state(&dq, 2, 4, 6, 8);

    // Split across the end; the bytes between end and start are kept.
    dq.push_back_zero(4);
    assert_state(&dq, 6, 4, 2, 8);
    assert_raw(&dq, 0, b"\0\0cdef\0\0");
    assert_front(&dq, b"ef\0\0\0\0");

    dq.push_front_zero(2);
    assert_state(&dq, 8, 2, 2, 8);
    assert_front(&dq, b"\0\0ef\0\0\0\0");
    dq.free();

    dq.push_front_zero(3);
    assert_state(&dq, 3, 0, 3, 3);
    assert_raw(&dq, 0, b"\0\0\0");
    dq.free();

    make_abcdef_in_8(&mut dq);
    dq.discard_front(2);
    dq.push_front_zero(4);
    assert_state(&dq, 8, 6, 6, 8);
    assert_raw(&dq, 0, b"\0\0cdef\0\0");
    assert_front(&dq, b"\0\0\0\0cdef");
    dq.free();

    // Into an empty deque from the back, with growth.
    dq.push_back(b"a");
    dq.push_back_zero(2);
    assert_state(&dq, 3, 0, 3, 3);
    assert_front(&dq, b"a\0\0");
    dq.free();
}

#[test]
fn peek_and_pop_back() {
    let mut dq = Deque::new();

    dq.push_back(b"abcdefgh");
    let mut buf3 = [0u8; 3];
    dq.peek_back(&mut buf3);
    assert_eq!(&buf3, b"fgh");
    dq.discard_back(3);
    assert_state(&dq, 5, 0, 5, 8);
    let mut buf5 = [0u8; 5];
    dq.pop_back(&mut buf5);
    assert_eq!(&buf5, b"abcde");
    assert_state(&dq, 0, 0, 0, 8);
    dq.free();

    // Wrapped: raw "WXcdefUV", content "efUVWX".
    make_abcdef_in_8(&mut dq);
    dq.discard_front(4);
    dq.push_back(b"UVWX");
    assert_state(&dq, 6, 4, 2, 8);

    let mut buf4 = [0u8; 4];
    dq.peek_back(&mut buf4);
    assert_eq!(&buf4, b"UVWX");

    // characterized, not endorsed: popping exactly end_pos bytes leaves
    // end_pos at capacity, not 0.
    let mut buf2 = [0u8; 2];
    dq.pop_back(&mut buf2);
    assert_eq!(&buf2, b"WX");
    assert_state(&dq, 4, 4, 8, 8);

    dq.peek_back(&mut buf2);
    assert_eq!(&buf2, b"UV");
    dq.discard_back(3);
    assert_state(&dq, 1, 4, 5, 8);
    assert_front(&dq, b"e");

    // A null destination only moves the positions. The safe API has no
    // null destination (`discard_*` is the null pop, see above); a peek of
    // the same size leaves the state alone.
    let mut buf1 = [0u8; 1];
    dq.peek_back(&mut buf1);
    assert_eq!(&buf1, b"e");
    dq.peek_front(&mut buf1);
    assert_eq!(&buf1, b"e");
    assert_state(&dq, 1, 4, 5, 8);
    dq.free();
}

#[test]
fn data_index() {
    let mut dq = Deque::new();
    make_abcdef_in_8(&mut dq);
    dq.discard_front(4);
    dq.push_back(b"UVWX");

    // Indexes are relative to start_pos and wrap at the capacity.
    assert_eq!(dq.data_offset(0), Some(4));
    assert_eq!(dq.raw()[dq.data_offset(0).unwrap()], b'e');
    assert_eq!(dq.raw()[dq.data_offset(3).unwrap()], b'V');
    assert_eq!(dq.data_offset(4), Some(0));
    assert_eq!(dq.raw()[dq.data_offset(5).unwrap()], b'X');
    assert_eq!(dq.data_offset(6), None);
    assert_eq!(dq.data_offset(usize::MAX), None);
    dq.free();
}

#[test]
fn place_and_upsize() {
    let mut dq = Deque::new();
    make_abcdef_in_8(&mut dq);
    dq.discard_front(4);
    assert_state(&dq, 2, 4, 6, 8);

    // Past the end: upsize zero-fills, then the copy wraps.
    dq.place(1, b"PQRST");
    assert_state(&dq, 6, 4, 2, 8);
    assert_raw(&dq, 0, b"STcdePQR");
    assert_front(&dq, b"ePQRST");

    // Inside: an overwrite, no size change.
    dq.place(0, b"Z");
    assert_state(&dq, 6, 4, 2, 8);
    assert_front(&dq, b"ZPQRST");

    // Shrinking or equal upsize is a no-op.
    dq.upsize(3);
    dq.upsize(6);
    assert_state(&dq, 6, 4, 2, 8);

    // A gap: upsize grows (moving the wrapped tail) and zero-fills it.
    dq.place(8, b"!");
    assert_state(&dq, 9, 12, 5, 16);
    assert_front(&dq, b"ZPQRST\0\0!");
    dq.free();

    // upsize on an empty deque allocates and zero-fills.
    dq.upsize(4);
    assert_state(&dq, 4, 0, 4, 4);
    assert_front(&dq, b"\0\0\0\0");
    dq.free();
}

#[test]
fn free_resets() {
    let (mut dq, allocs) = counted();

    // Freeing a deque that never allocated is fine and frees nothing.
    dq.free();
    assert!(dq.raw().is_empty());
    assert_state(&dq, 0, 0, 0, 0);
    assert_eq!(allocs.get(), 0);

    make_full_wrapped(&mut dq);
    assert_eq!(allocs.get(), 1);
    dq.free();
    assert!(dq.raw().is_empty());
    assert_state(&dq, 0, 0, 0, 0);
    assert_eq!(allocs.get(), 0);

    // A freed deque is reusable.
    dq.push_back(b"ab");
    assert_state(&dq, 2, 0, 2, 2);
    dq.free();
    assert_eq!(allocs.get(), 0);
}

/// The obs-output delay pattern: fixed-size records pushed at the back,
/// peeked and popped at the front, through growth and wraparound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Record {
    ts: u64,
    seq: u32,
    kind: i32,
}

const RECORD_SIZE: usize = 16;

impl Record {
    fn to_bytes(self) -> [u8; RECORD_SIZE] {
        let mut b = [0u8; RECORD_SIZE];
        b[..8].copy_from_slice(&self.ts.to_ne_bytes());
        b[8..12].copy_from_slice(&self.seq.to_ne_bytes());
        b[12..].copy_from_slice(&self.kind.to_ne_bytes());
        b
    }

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            ts: u64::from_ne_bytes(b[..8].try_into().unwrap()),
            seq: u32::from_ne_bytes(b[8..12].try_into().unwrap()),
            kind: i32::from_ne_bytes(b[12..16].try_into().unwrap()),
        }
    }
}

/// Reads the record at byte `idx` the way C does: `memcpy` from
/// `deque_data(idx)`, a contiguous `RECORD_SIZE` bytes.
fn record_at<B: DequeBuf>(dq: &Deque<B>, idx: usize) -> Record {
    let off = dq.data_offset(idx).expect("index inside the size");
    Record::from_bytes(&dq.raw()[off..off + RECORD_SIZE])
}

#[test]
fn record_fifo() {
    let (mut dq, allocs) = counted();
    let mut next_push: u32 = 0;
    let mut next_pop: u32 = 0;
    let mut wrapped = 0usize;
    let mut max_capacity = 0usize;

    for round in 0u32..64 {
        let pushes = round % 3 + 1;
        let pops = round % 2 + 1;

        for _ in 0..pushes {
            let rec = Record {
                ts: u64::from(next_push) * 1000,
                seq: next_push,
                kind: (next_push % 4) as i32,
            };
            dq.push_back(&rec.to_bytes());
            next_push += 1;
        }

        // Every record is where deque_data says, in push order.
        for i in next_pop..next_push {
            let rec = record_at(&dq, (i - next_pop) as usize * RECORD_SIZE);
            assert_eq!(rec.seq, i);
            assert_eq!(rec.ts, u64::from(i) * 1000);
        }

        let mut i = 0;
        while i < pops && !dq.is_empty() {
            let mut peeked = [0u8; RECORD_SIZE];
            dq.peek_front(&mut peeked);
            let rec = Record::from_bytes(&peeked);
            assert_eq!(rec.seq, next_pop);
            if rec.kind == 1 {
                dq.discard_front(RECORD_SIZE);
            } else {
                let mut popped = [0u8; RECORD_SIZE];
                dq.pop_front(&mut popped);
                assert_eq!(popped, peeked);
            }
            next_pop += 1;
            i += 1;
        }

        assert_eq!(dq.size(), (next_push - next_pop) as usize * RECORD_SIZE);
        if dq.size() != 0 && dq.end_pos() <= dq.start_pos() {
            wrapped += 1;
        }
        max_capacity = max_capacity.max(dq.capacity());
    }

    // 127 pushes, 96 pops; the capacity doubled from one record (16 bytes)
    // to 64 records (1024 bytes), and the queue ended 36 rounds wrapped.
    assert_eq!(next_push, 127);
    assert_eq!(next_pop, 96);
    assert_eq!(dq.size(), 31 * RECORD_SIZE);
    assert_eq!(RECORD_SIZE, 16);
    assert_eq!(max_capacity, 64 * RECORD_SIZE);
    assert_eq!(dq.capacity(), max_capacity);
    assert_eq!(wrapped, 36);
    assert_eq!(allocs.get(), 1);

    while !dq.is_empty() {
        let mut popped = [0u8; RECORD_SIZE];
        dq.pop_front(&mut popped);
        assert_eq!(Record::from_bytes(&popped).seq, next_pop);
        next_pop += 1;
    }
    assert_eq!(next_pop, 127);
    assert_eq!(dq.start_pos(), 0);
    assert_eq!(dq.end_pos(), 0);

    dq.free();
    assert_eq!(allocs.get(), 0);
}

// New edge cases.

#[test]
fn zero_size_ops_on_empty_allocate_nothing() {
    let (mut dq, allocs) = counted();
    dq.push_back(&[]);
    dq.push_front(&[]);
    dq.push_back_zero(0);
    dq.push_front_zero(0);
    dq.place(0, &[]);
    dq.upsize(0);
    dq.discard_front(0);
    dq.discard_back(0);
    dq.peek_front(&mut []);
    dq.peek_back(&mut []);
    assert_state(&dq, 0, 0, 0, 0);
    assert_eq!(allocs.get(), 0);
}

#[test]
fn zero_size_place_past_the_end_upsizes() {
    let mut dq = Deque::new();
    dq.place(4, &[]);
    assert_state(&dq, 4, 0, 4, 4);
    assert_front(&dq, &[0; 4]);
}

#[test]
fn push_back_into_full_unwrapped_after_pop_wraps_to_zero() {
    // end_pos == capacity with a gap at the front: the next push that fits
    // goes entirely to position 0.
    let mut dq = Deque::new();
    dq.push_back(b"abcdefgh");
    dq.discard_front(3);
    assert_state(&dq, 5, 3, 8, 8);
    dq.push_back(b"12");
    assert_state(&dq, 7, 3, 2, 8);
    assert_raw(&dq, 0, b"12");
    assert_front(&dq, b"defgh12");
}

#[test]
fn pop_back_across_the_wrap() {
    let mut dq = Deque::new();
    make_full_wrapped(&mut dq);
    let mut out = [0u8; 5];
    dq.pop_back(&mut out);
    assert_eq!(&out, b"ghXYZ");
    assert_state(&dq, 3, 3, 6, 8);
    assert_front(&dq, b"def");
}

#[test]
fn peek_front_across_the_wrap() {
    let mut dq = Deque::new();
    make_full_wrapped(&mut dq);
    let mut out = [0u8; 8];
    dq.peek_front(&mut out);
    assert_eq!(&out, b"defghXYZ");
    // Peeking does not move anything.
    assert_state(&dq, 8, 3, 3, 8);
}

#[test]
fn pop_back_to_empty_resets_positions() {
    let mut dq = Deque::new();
    make_full_wrapped(&mut dq);
    dq.discard_back(8);
    assert_state(&dq, 0, 0, 0, 8);
    dq.push_front(b"q");
    assert_state(&dq, 1, 0, 1, 8);
}

#[test]
fn reserve_on_unwrapped_offset_does_not_move() {
    let mut dq = Deque::new();
    dq.push_back(b"abcdefgh");
    dq.discard_front(2);
    dq.reserve(20);
    assert_state(&dq, 6, 2, 8, 20);
    assert_raw(&dq, 2, b"cdefgh");
}

#[test]
fn reserve_on_empty_with_buffer_keeps_positions() {
    let mut dq = Deque::new();
    dq.push_back(b"abcd");
    dq.discard_front(4);
    dq.reserve(9);
    assert_state(&dq, 0, 0, 0, 9);
    dq.push_back(b"x");
    assert_state(&dq, 1, 0, 1, 9);
}

#[test]
fn place_wrapping_inside_the_size() {
    let mut dq = Deque::new();
    make_full_wrapped(&mut dq);
    dq.place(3, b"12345");
    assert_state(&dq, 8, 3, 3, 8);
    assert_front(&dq, b"def12345");
    assert_raw(&dq, 0, b"345def12");
}

#[test]
#[should_panic(expected = "deque_peek_front past the size")]
fn peek_front_past_the_size_panics() {
    let mut dq = Deque::new();
    dq.push_back(b"ab");
    dq.peek_front(&mut [0u8; 3]);
}

#[test]
#[should_panic(expected = "deque_pop_back past the size")]
fn discard_back_past_the_size_panics() {
    let mut dq = Deque::new();
    dq.push_back(b"ab");
    dq.discard_back(3);
}

#[test]
fn vec_buf_free_releases_the_bytes() {
    let mut dq: Deque<VecBuf> = Deque::new();
    dq.push_back(b"abc");
    dq.free();
    assert!(dq.raw().is_empty());
    assert!(dq.buf().bytes().is_empty());
}
