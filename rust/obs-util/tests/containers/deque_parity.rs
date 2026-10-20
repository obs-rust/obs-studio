//! Tier 3: the Rust `deque` functions agree with the original C `deque.h`
//! (compiled into the oracle as non-inline `oracle_deque_*` wrappers) on
//! arbitrary operation sequences.
//!
//! - `deque_ops_match_c`: twin deques, one driven by the Rust functions
//!   ([`obs_util::ffi::deque`]) and one by the C, plus the safe
//!   [`Deque`] over a `Vec`. After every step the size fields, the null-ness
//!   of the buffer, every live byte at its buffer offset, and each peek, pop
//!   and `deque_data` result must be equal. Bytes outside the live range are
//!   not compared: C leaves grown memory uninitialized.
//! - `deque_mixed_in_place_matches_c`: one C `struct deque` that each step
//!   hands to the Rust or the C functions at random (including `deque_free`),
//!   against a C-only twin. This is the obs-output use: Rust pushes and pops a
//!   deque C allocated, and C frees what Rust allocated (both through bmem).
//! - `bmem_accounting_matches_c`: the `bnum_allocs()` deltas of the
//!   `test_deque` accounting cases through the Rust functions, in its own
//!   process (`common::isolated`, #1133).
//!
//! Intentional differences: none. Peek and pop sizes stay within the size
//! (C asserts).

use core::ffi::c_void;
use core::ptr;
use core::slice;

use obs_c_oracle::deque::{self as c, OracleDeque};
use obs_util::deque::{Deque, DequeState};
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::deque::{self as r, deque};
use proptest::collection::vec;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    PushBack(Vec<u8>),
    PushFront(Vec<u8>),
    PushBackZero(usize),
    PushFrontZero(usize),
    /// Size as a fraction of the current size (`n % (size + 1)`); `true`
    /// pops into a buffer, `false` passes a null destination.
    PopFront(usize, bool),
    PopBack(usize, bool),
    PeekFront(usize, bool),
    PeekBack(usize, bool),
    Reserve(usize),
    Upsize(usize),
    Place(usize, Vec<u8>),
    Data(usize),
    Free,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => vec(any::<u8>(), 0..=40).prop_map(Op::PushBack),
        3 => vec(any::<u8>(), 0..=40).prop_map(Op::PushFront),
        1 => (0usize..=24).prop_map(Op::PushBackZero),
        1 => (0usize..=24).prop_map(Op::PushFrontZero),
        5 => (any::<usize>(), any::<bool>()).prop_map(|(n, o)| Op::PopFront(n, o)),
        3 => (any::<usize>(), any::<bool>()).prop_map(|(n, o)| Op::PopBack(n, o)),
        2 => (any::<usize>(), any::<bool>()).prop_map(|(n, o)| Op::PeekFront(n, o)),
        2 => (any::<usize>(), any::<bool>()).prop_map(|(n, o)| Op::PeekBack(n, o)),
        2 => (0usize..=160).prop_map(Op::Reserve),
        1 => (0usize..=120).prop_map(Op::Upsize),
        2 => ((0usize..=80), vec(any::<u8>(), 0..=24)).prop_map(|(p, d)| Op::Place(p, d)),
        2 => any::<usize>().prop_map(Op::Data),
        1 => Just(Op::Free),
    ]
}

/// What a step returned: peeked or popped bytes, or a `deque_data` offset.
#[derive(Debug, PartialEq, Eq)]
enum Out {
    Nothing,
    Bytes(Option<Vec<u8>>),
    Offset(Option<usize>),
}

/// An op with its sizes resolved against the current size.
fn resolve(op: &Op, size: usize) -> Op {
    let frac = |n: usize| n % (size + 1);
    match *op {
        Op::PopFront(n, o) => Op::PopFront(frac(n), o),
        Op::PopBack(n, o) => Op::PopBack(frac(n), o),
        Op::PeekFront(n, o) => Op::PeekFront(frac(n), o),
        Op::PeekBack(n, o) => Op::PeekBack(frac(n), o),
        // Indexes up to two past the size, and the far end.
        Op::Data(i) if i.is_multiple_of(7) => Op::Data(usize::MAX),
        Op::Data(i) => Op::Data(i % (size + 3)),
        _ => op.clone(),
    }
}

fn offset(base: *mut c_void, p: *mut c_void) -> Option<usize> {
    (!p.is_null()).then(|| p as usize - base as usize)
}

/// The C wrappers' signatures, so one driver serves both implementations.
struct Impl {
    free: unsafe fn(*mut deque),
    reserve: unsafe fn(*mut deque, usize),
    upsize: unsafe fn(*mut deque, usize),
    place: unsafe fn(*mut deque, usize, *const c_void, usize),
    push_back: unsafe fn(*mut deque, *const c_void, usize),
    push_front: unsafe fn(*mut deque, *const c_void, usize),
    push_back_zero: unsafe fn(*mut deque, usize),
    push_front_zero: unsafe fn(*mut deque, usize),
    peek_front: unsafe fn(*mut deque, *mut c_void, usize),
    peek_back: unsafe fn(*mut deque, *mut c_void, usize),
    pop_front: unsafe fn(*mut deque, *mut c_void, usize),
    pop_back: unsafe fn(*mut deque, *mut c_void, usize),
    data: unsafe fn(*mut deque, usize) -> *mut c_void,
}

const RUST: Impl = Impl {
    free: r::deque_free,
    reserve: r::deque_reserve,
    upsize: r::deque_upsize,
    place: r::deque_place,
    push_back: r::deque_push_back,
    push_front: r::deque_push_front,
    push_back_zero: r::deque_push_back_zero,
    push_front_zero: r::deque_push_front_zero,
    peek_front: r::deque_peek_front,
    peek_back: r::deque_peek_back,
    pop_front: r::deque_pop_front,
    pop_back: r::deque_pop_back,
    data: r::deque_data,
};

// SAFETY (all `c_*` below): `deque` and `OracleDeque` have the same layout
// (`deque_layout.rs`), so the pointer cast reinterprets nothing.
unsafe fn c_free(dq: *mut deque) {
    unsafe { c::oracle_deque_free(dq.cast()) }
}
unsafe fn c_reserve(dq: *mut deque, n: usize) {
    unsafe { c::oracle_deque_reserve(dq.cast(), n) }
}
unsafe fn c_upsize(dq: *mut deque, n: usize) {
    unsafe { c::oracle_deque_upsize(dq.cast(), n) }
}
unsafe fn c_place(dq: *mut deque, p: usize, d: *const c_void, n: usize) {
    unsafe { c::oracle_deque_place(dq.cast(), p, d, n) }
}
unsafe fn c_push_back(dq: *mut deque, d: *const c_void, n: usize) {
    unsafe { c::oracle_deque_push_back(dq.cast(), d, n) }
}
unsafe fn c_push_front(dq: *mut deque, d: *const c_void, n: usize) {
    unsafe { c::oracle_deque_push_front(dq.cast(), d, n) }
}
unsafe fn c_push_back_zero(dq: *mut deque, n: usize) {
    unsafe { c::oracle_deque_push_back_zero(dq.cast(), n) }
}
unsafe fn c_push_front_zero(dq: *mut deque, n: usize) {
    unsafe { c::oracle_deque_push_front_zero(dq.cast(), n) }
}
unsafe fn c_peek_front(dq: *mut deque, d: *mut c_void, n: usize) {
    unsafe { c::oracle_deque_peek_front(dq.cast(), d, n) }
}
unsafe fn c_peek_back(dq: *mut deque, d: *mut c_void, n: usize) {
    unsafe { c::oracle_deque_peek_back(dq.cast(), d, n) }
}
unsafe fn c_pop_front(dq: *mut deque, d: *mut c_void, n: usize) {
    unsafe { c::oracle_deque_pop_front(dq.cast(), d, n) }
}
unsafe fn c_pop_back(dq: *mut deque, d: *mut c_void, n: usize) {
    unsafe { c::oracle_deque_pop_back(dq.cast(), d, n) }
}
unsafe fn c_data(dq: *mut deque, i: usize) -> *mut c_void {
    unsafe { c::oracle_deque_data(dq.cast(), i) }
}

const C: Impl = Impl {
    free: c_free,
    reserve: c_reserve,
    upsize: c_upsize,
    place: c_place,
    push_back: c_push_back,
    push_front: c_push_front,
    push_back_zero: c_push_back_zero,
    push_front_zero: c_push_front_zero,
    peek_front: c_peek_front,
    peek_back: c_peek_back,
    pop_front: c_pop_front,
    pop_back: c_pop_back,
    data: c_data,
};

/// Applies a resolved op to `dq` through `imp`.
///
/// # Safety
///
/// `dq` is a valid deque that only these functions have touched, and the
/// op's sizes are within its size.
unsafe fn apply(imp: &Impl, dq: *mut deque, op: &Op) -> Out {
    let read = |n: usize, out: bool, f: unsafe fn(*mut deque, *mut c_void, usize)| {
        if out {
            // A guard byte past the end catches an overlong copy.
            let mut buf = vec![0xa5u8; n + 1];
            unsafe { f(dq, buf.as_mut_ptr().cast(), n) };
            assert_eq!(buf.pop(), Some(0xa5), "copy past the requested size");
            Out::Bytes(Some(buf))
        } else {
            unsafe { f(dq, ptr::null_mut(), n) };
            Out::Bytes(None)
        }
    };
    unsafe {
        match op {
            Op::PushBack(d) => (imp.push_back)(dq, d.as_ptr().cast(), d.len()),
            Op::PushFront(d) => (imp.push_front)(dq, d.as_ptr().cast(), d.len()),
            Op::PushBackZero(n) => (imp.push_back_zero)(dq, *n),
            Op::PushFrontZero(n) => (imp.push_front_zero)(dq, *n),
            Op::PopFront(n, o) => return read(*n, *o, imp.pop_front),
            Op::PopBack(n, o) => return read(*n, *o, imp.pop_back),
            Op::PeekFront(n, o) => return read(*n, *o, imp.peek_front),
            Op::PeekBack(n, o) => return read(*n, *o, imp.peek_back),
            Op::Reserve(n) => (imp.reserve)(dq, *n),
            Op::Upsize(n) => (imp.upsize)(dq, *n),
            Op::Place(p, d) => (imp.place)(dq, *p, d.as_ptr().cast(), d.len()),
            Op::Data(i) => return Out::Offset(offset((*dq).data, (imp.data)(dq, *i))),
            Op::Free => (imp.free)(dq),
        }
    }
    Out::Nothing
}

/// The same op on the safe core.
fn apply_safe(s: &mut Deque, op: &Op) -> Out {
    let bytes = |n: usize, out: bool, f: &mut dyn FnMut(&mut [u8])| {
        let mut buf = vec![0u8; n];
        f(&mut buf);
        Out::Bytes(out.then_some(buf))
    };
    match op {
        Op::PushBack(d) => s.push_back(d),
        Op::PushFront(d) => s.push_front(d),
        Op::PushBackZero(n) => s.push_back_zero(*n),
        Op::PushFrontZero(n) => s.push_front_zero(*n),
        Op::PopFront(n, o) => return bytes(*n, *o, &mut |b| s.pop_front(b)),
        Op::PopBack(n, o) => return bytes(*n, *o, &mut |b| s.pop_back(b)),
        Op::PeekFront(n, o) => return bytes(*n, *o, &mut |b| s.peek_front(b)),
        Op::PeekBack(n, o) => return bytes(*n, *o, &mut |b| s.peek_back(b)),
        Op::Reserve(n) => s.reserve(*n),
        Op::Upsize(n) => s.upsize(*n),
        Op::Place(p, d) => s.place(*p, d),
        Op::Data(i) => return Out::Offset(s.data_offset(*i)),
        Op::Free => s.free(),
    }
    Out::Nothing
}

fn state(d: &deque) -> DequeState {
    DequeState {
        size: d.size,
        start_pos: d.start_pos,
        end_pos: d.end_pos,
        capacity: d.capacity,
    }
}

/// The live bytes, each read at its buffer offset (front to back).
fn live(data: *const u8, st: DequeState) -> Vec<u8> {
    if st.size == 0 {
        return Vec::new();
    }
    // SAFETY: a deque with live bytes owns a `capacity`-byte buffer.
    let buf = unsafe { slice::from_raw_parts(data, st.capacity) };
    (0..st.size)
        .map(|i| buf[(st.start_pos + i) % st.capacity])
        .collect()
}

fn check(a: &deque, b: &deque, safe: Option<&Deque>) -> Result<(), TestCaseError> {
    let st = state(b);
    prop_assert_eq!(state(a), st);
    prop_assert_eq!(a.data.is_null(), b.data.is_null());
    let lb = live(b.data.cast(), st);
    prop_assert_eq!(live(a.data.cast(), st), lb.clone());
    if let Some(s) = safe {
        prop_assert_eq!(s.state(), st);
        prop_assert_eq!(s.raw().is_empty(), b.data.is_null());
        prop_assert_eq!(live(s.raw().as_ptr(), st), lb);
    }
    Ok(())
}

fn empty() -> deque {
    deque {
        data: ptr::null_mut(),
        size: 0,
        start_pos: 0,
        end_pos: 0,
        capacity: 0,
    }
}

fn run_twins(ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut a = empty();
    let mut b = OracleDeque {
        data: ptr::null_mut(),
        size: 0,
        start_pos: 0,
        end_pos: 0,
        capacity: 0,
    };
    // SAFETY: `b` is a valid, writable deque.
    unsafe { c::oracle_deque_init(&mut b) };
    let b: *mut deque = (&mut b as *mut OracleDeque).cast();
    let mut s = Deque::new();

    for op in &ops {
        // SAFETY: `b` points to the live oracle deque above.
        let op = resolve(op, unsafe { (*b).size });
        // SAFETY: both deques are valid and only these functions touch
        // them; `resolve` keeps sizes within the size.
        let (ra, rb) = unsafe { (apply(&RUST, &mut a, &op), apply(&C, b, &op)) };
        prop_assert_eq!(&ra, &rb, "{:?}", op);
        prop_assert_eq!(apply_safe(&mut s, &op), rb, "{:?}", op);
        // SAFETY: as above.
        check(&a, unsafe { &*b }, Some(&s))?;
    }

    // SAFETY: both buffers are bmem blocks (or null).
    unsafe {
        r::deque_free(&mut a);
        c_free(b);
    }
    Ok(())
}

fn run_mixed(ops: Vec<(Op, bool)>) -> Result<(), TestCaseError> {
    let mut m = empty();
    let mut reference = empty();
    // SAFETY: both are valid, writable deques.
    unsafe {
        c::oracle_deque_init((&mut m as *mut deque).cast());
        r::deque_init(&mut reference);
    }

    for (op, rust) in &ops {
        let op = resolve(op, reference.size);
        let imp = if *rust { &RUST } else { &C };
        // SAFETY: both deques are valid; each step hands `m` to one complete
        // implementation, and both use the same bmem allocator.
        let (rm, rr) = unsafe { (apply(imp, &mut m, &op), apply(&C, &mut reference, &op)) };
        prop_assert_eq!(rm, rr, "{:?} rust={}", op, rust);
        check(&m, &reference, None)?;
    }

    // SAFETY: as above.
    unsafe {
        r::deque_free(&mut m);
        c_free(&mut reference);
    }
    Ok(())
}

proptest! {
    #[test]
    fn deque_ops_match_c(ops in vec(op(), 0..64)) {
        run_twins(ops)?;
    }

    #[test]
    fn deque_mixed_in_place_matches_c(ops in vec((op(), any::<bool>()), 0..64)) {
        run_mixed(ops)?;
    }
}

/// `test_deque`'s fixed-size record FIFO, through the Rust and C functions
/// in turn on one deque, against the C alone.
#[test]
fn record_fifo_matches_c() {
    let mut ops = Vec::new();
    let mut seq: u32 = 0;
    for round in 0u32..64 {
        for _ in 0..round % 3 + 1 {
            let mut rec = [0u8; 16];
            rec[..8].copy_from_slice(&(u64::from(seq) * 1000).to_ne_bytes());
            rec[8..12].copy_from_slice(&seq.to_ne_bytes());
            rec[12..].copy_from_slice(&((seq % 4) as i32).to_ne_bytes());
            ops.push((Op::PushBack(rec.to_vec()), round.is_multiple_of(2)));
            seq += 1;
        }
        for _ in 0..round % 2 + 1 {
            ops.push((Op::PeekFront(16, true), round.is_multiple_of(3)));
            ops.push((Op::PopFront(16, seq.is_multiple_of(2)), round % 4 < 2));
        }
    }
    // `resolve` takes the size modulo `size + 1`, which keeps 16 when 16 or
    // more bytes are live and pops less only once the queue runs dry.
    run_mixed(ops).unwrap();
}

/// The bmem accounting of the Rust functions, as `test_deque` pins it for C:
/// the first push allocates, growth reallocates the same block, popping to
/// empty keeps it, and `deque_free` from either side releases it.
fn measure_allocs() {
    // SAFETY: plain counter reads and deques that only these functions touch.
    unsafe {
        let base = bnum_allocs();
        let mut dq = empty();
        r::deque_init(&mut dq);
        r::deque_free(&mut dq);
        assert_eq!(bnum_allocs(), base);

        r::deque_push_back(&mut dq, b"abcd".as_ptr().cast(), 4);
        assert_eq!(
            state(&dq),
            DequeState {
                size: 4,
                start_pos: 0,
                end_pos: 4,
                capacity: 4
            }
        );
        assert_eq!(bnum_allocs(), base + 1);
        r::deque_push_back(&mut dq, [b'z'; 40].as_ptr().cast(), 40);
        assert_eq!(dq.capacity, 44);
        r::deque_pop_front(&mut dq, ptr::null_mut(), 44);
        assert_eq!(
            state(&dq),
            DequeState {
                size: 0,
                start_pos: 0,
                end_pos: 0,
                capacity: 44
            }
        );
        assert!(!dq.data.is_null());
        assert_eq!(bnum_allocs(), base + 1);
        r::deque_free(&mut dq);
        assert!(dq.data.is_null());
        assert_eq!(bnum_allocs(), base);

        r::deque_reserve(&mut dq, 0);
        assert_eq!(bnum_allocs(), base);
        r::deque_reserve(&mut dq, 16);
        assert_eq!(bnum_allocs(), base + 1);

        // C frees what Rust allocated, and Rust what C allocated.
        c_free(&mut dq);
        assert_eq!(bnum_allocs(), base);
        c_push_back(&mut dq, b"ab".as_ptr().cast(), 2);
        r::deque_push_back(&mut dq, b"cdefg".as_ptr().cast(), 5);
        assert_eq!(bnum_allocs(), base + 1);
        r::deque_free(&mut dq);
        assert_eq!(bnum_allocs(), base);
    }
}

const BMEM_ACCOUNTING: &str = concat!(module_path!(), "::bmem_accounting_matches_c");

#[test]
fn bmem_accounting_matches_c() {
    if crate::common::isolated(BMEM_ACCOUNTING) {
        measure_allocs();
    }
}
