//! Tier 3: the Rust `darray` functions agree with the original C
//! `darray.h` (compiled into the oracle as non-inline `oracle_*` wrappers)
//! on arbitrary operation sequences.
//!
//! - `darray_ops_match_c`: twin arrays, one driven by the Rust functions
//!   ([`obs_util::ffi::darray`]) and one by the C, plus the safe [`DArray`]
//!   for one-byte items. The operations are every function the Rust side
//!   has, including the interleaver's (#1594): insert, erase range, pop
//!   front, push back and push back new (C through the function or the
//!   GCC/Clang `da_push_back_new` macro at random). After every step `num`,
//!   `capacity`, the null-ness of the buffer, each return value (a push back
//!   index, the offset of the item push back new returns) and every
//!   determined byte must be equal.
//! - `darray_mixed_in_place_matches_c`: one `struct darray` that each step
//!   hands to the Rust or the C functions at random (`darray_free`
//!   included), against a C-only twin. This is the interleaver's use: Rust
//!   inserts into and erases from arrays that C allocated, and C frees what
//!   Rust allocated (both through bmem).
//! - `interleaver_patterns_match_c`: the `test_darray_ops` resort and
//!   packet-time patterns, run the same mixed way.
//! - `bmem_accounting_matches_c`: the `bnum_allocs()` deltas of a hand-off
//!   in both directions, in its own process (`common::isolated`, #1133).
//!
//! "Determined bytes" are the slots below `known`, which the test tracks:
//! every slot below `num`, and also the slots past `num` that removal left
//! behind (characterized, not endorsed: removal does not clear them, and
//! `darray_ensure_capacity` copies the whole old capacity, so they survive
//! growth). `darray_reserve` copies only `num` items, so a reserve that
//! reallocates drops `known` to `num`. Other bytes are not compared: C
//! leaves new memory uninitialized.
//!
//! Intentional differences: none. Indices and ranges stay within what the
//! header asserts.

use core::ffi::c_void;
use core::ptr;
use core::slice;

use obs_c_oracle::darray::{self as c, OracleDarray};
use obs_util::darray::DArray;
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::darray::{self as r, darray};
use proptest::collection::vec;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    PushBackArray(Vec<u8>),
    Reserve(usize),
    EnsureCapacity(usize),
    Resize(usize),
    Clear,
    /// Index modulo `num`.
    Erase(usize),
    PopBack,
    /// One item: the first `es` bytes.
    PushBack([u8; 16]),
    /// `true`: C through the `da_push_back_new` macro, `false`: the
    /// function. Rust has one function for both.
    PushBackNew(bool),
    /// Index modulo `num + 1`.
    Insert(usize, [u8; 16]),
    /// Start modulo `num`, then a length of at least one that fits.
    EraseRange(usize, usize),
    PopFront,
    Free,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => vec(any::<u8>(), 0..=96).prop_map(Op::PushBackArray),
        1 => (0usize..64).prop_map(Op::Reserve),
        1 => (0usize..64).prop_map(Op::EnsureCapacity),
        1 => (0usize..64).prop_map(Op::Resize),
        1 => Just(Op::Clear),
        2 => any::<usize>().prop_map(Op::Erase),
        1 => Just(Op::PopBack),
        3 => any::<[u8; 16]>().prop_map(Op::PushBack),
        3 => any::<bool>().prop_map(Op::PushBackNew),
        5 => (any::<usize>(), any::<[u8; 16]>()).prop_map(|(i, d)| Op::Insert(i, d)),
        3 => (any::<usize>(), any::<usize>()).prop_map(|(s, n)| Op::EraseRange(s, n)),
        2 => Just(Op::PopFront),
        1 => Just(Op::Free),
    ]
}

/// An op with its indices resolved against `num`; `None` when the header
/// does not allow it on an array of `num` items.
fn resolve(op: &Op, num: usize) -> Option<Op> {
    Some(match *op {
        Op::Erase(i) if num > 0 => Op::Erase(i % num),
        Op::Erase(_) => return None,
        Op::PopBack | Op::PopFront if num == 0 => return None,
        Op::Insert(i, d) => Op::Insert(i % (num + 1), d),
        Op::EraseRange(s, n) if num > 0 => {
            let start = s % num;
            let end = start + 1 + n % (num - start);
            Op::EraseRange(start, end)
        }
        Op::EraseRange(..) => return None,
        _ => op.clone(),
    })
}

/// What a step returned.
#[derive(Debug, PartialEq, Eq)]
enum Out {
    Nothing,
    Index(usize),
    /// Push back new: the item's byte offset in the buffer after the step.
    Item(usize),
}

/// One implementation of the functions, all on `*mut darray`.
struct Impl {
    free: unsafe fn(*mut darray),
    reserve: unsafe fn(usize, *mut darray, usize),
    ensure_capacity: unsafe fn(usize, *mut darray, usize),
    resize: unsafe fn(usize, *mut darray, usize),
    clear: unsafe fn(*mut darray),
    push_back_array: unsafe fn(usize, *mut darray, *const c_void, usize) -> usize,
    erase: unsafe fn(usize, *mut darray, usize),
    pop_back: unsafe fn(usize, *mut darray),
    push_back: unsafe fn(usize, *mut darray, *const c_void) -> usize,
    push_back_new: unsafe fn(usize, *mut darray, bool) -> *mut c_void,
    insert: unsafe fn(usize, *mut darray, usize, *const c_void),
    erase_range: unsafe fn(usize, *mut darray, usize, usize),
    pop_front: unsafe fn(usize, *mut darray),
}

unsafe fn rust_push_back_new(es: usize, d: *mut darray, _macro: bool) -> *mut c_void {
    // SAFETY: forwarded contract.
    unsafe { r::darray_push_back_new(es, d) }
}

const RUST: Impl = Impl {
    free: r::darray_free,
    reserve: r::darray_reserve,
    ensure_capacity: r::darray_ensure_capacity,
    resize: r::darray_resize,
    clear: r::darray_clear,
    push_back_array: r::darray_push_back_array,
    erase: r::darray_erase,
    pop_back: r::darray_pop_back,
    push_back: r::darray_push_back,
    push_back_new: rust_push_back_new,
    insert: r::darray_insert,
    erase_range: r::darray_erase_range,
    pop_front: r::darray_pop_front,
};

// SAFETY (all `c_*` below): `darray` and `OracleDarray` have the same layout
// (`darray_layout.rs`), so the pointer cast reinterprets nothing.
unsafe fn c_free(d: *mut darray) {
    unsafe { c::oracle_darray_free(d.cast()) }
}
unsafe fn c_reserve(es: usize, d: *mut darray, n: usize) {
    unsafe { c::oracle_darray_reserve(es, d.cast(), n) }
}
unsafe fn c_ensure_capacity(es: usize, d: *mut darray, n: usize) {
    unsafe { c::oracle_darray_ensure_capacity(es, d.cast(), n) }
}
unsafe fn c_resize(es: usize, d: *mut darray, n: usize) {
    unsafe { c::oracle_darray_resize(es, d.cast(), n) }
}
unsafe fn c_clear(d: *mut darray) {
    unsafe { c::oracle_darray_clear(d.cast()) }
}
unsafe fn c_push_back_array(es: usize, d: *mut darray, a: *const c_void, n: usize) -> usize {
    unsafe { c::oracle_darray_push_back_array(es, d.cast(), a, n) }
}
unsafe fn c_erase(es: usize, d: *mut darray, i: usize) {
    unsafe { c::oracle_darray_erase(es, d.cast(), i) }
}
unsafe fn c_pop_back(es: usize, d: *mut darray) {
    unsafe { c::oracle_darray_pop_back(es, d.cast()) }
}
unsafe fn c_push_back(es: usize, d: *mut darray, item: *const c_void) -> usize {
    unsafe { c::oracle_darray_push_back(es, d.cast(), item) }
}
unsafe fn c_push_back_new(es: usize, d: *mut darray, macro_: bool) -> *mut c_void {
    unsafe {
        if macro_ {
            let p = c::oracle_da_push_back_new(es, d.cast());
            assert!(!p.is_null(), "no DARRAY type for {es}-byte items");
            p
        } else {
            c::oracle_darray_push_back_new(es, d.cast())
        }
    }
}
unsafe fn c_insert(es: usize, d: *mut darray, i: usize, item: *const c_void) {
    unsafe { c::oracle_darray_insert(es, d.cast(), i, item) }
}
unsafe fn c_erase_range(es: usize, d: *mut darray, s: usize, e: usize) {
    unsafe { c::oracle_darray_erase_range(es, d.cast(), s, e) }
}
unsafe fn c_pop_front(es: usize, d: *mut darray) {
    unsafe { c::oracle_darray_pop_front(es, d.cast()) }
}

const C: Impl = Impl {
    free: c_free,
    reserve: c_reserve,
    ensure_capacity: c_ensure_capacity,
    resize: c_resize,
    clear: c_clear,
    push_back_array: c_push_back_array,
    erase: c_erase,
    pop_back: c_pop_back,
    push_back: c_push_back,
    push_back_new: c_push_back_new,
    insert: c_insert,
    erase_range: c_erase_range,
    pop_front: c_pop_front,
};

/// Applies a resolved op to `d` through `imp`.
///
/// # Safety
///
/// `d` is a valid darray of `es`-byte items that only these functions have
/// touched, and the op's indices are resolved against its `num`.
unsafe fn apply(imp: &Impl, es: usize, d: *mut darray, op: &Op) -> Out {
    // SAFETY: the caller's contract; item and array inputs are valid for
    // their length.
    unsafe {
        match op {
            Op::PushBackArray(data) => {
                let n = data.len() / es;
                Out::Index((imp.push_back_array)(es, d, data.as_ptr().cast(), n))
            }
            Op::Reserve(n) => {
                (imp.reserve)(es, d, *n);
                Out::Nothing
            }
            Op::EnsureCapacity(n) => {
                (imp.ensure_capacity)(es, d, *n);
                Out::Nothing
            }
            Op::Resize(n) => {
                (imp.resize)(es, d, *n);
                Out::Nothing
            }
            Op::Clear => {
                (imp.clear)(d);
                Out::Nothing
            }
            Op::Erase(i) => {
                (imp.erase)(es, d, *i);
                Out::Nothing
            }
            Op::PopBack => {
                (imp.pop_back)(es, d);
                Out::Nothing
            }
            Op::PushBack(item) => Out::Index((imp.push_back)(es, d, item.as_ptr().cast())),
            Op::PushBackNew(macro_) => {
                let p = (imp.push_back_new)(es, d, *macro_);
                Out::Item(p as usize - (*d).array as usize)
            }
            Op::Insert(i, item) => {
                (imp.insert)(es, d, *i, item.as_ptr().cast());
                Out::Nothing
            }
            Op::EraseRange(s, e) => {
                (imp.erase_range)(es, d, *s, *e);
                Out::Nothing
            }
            Op::PopFront => {
                (imp.pop_front)(es, d);
                Out::Nothing
            }
            Op::Free => {
                (imp.free)(d);
                Out::Nothing
            }
        }
    }
}

/// The same op on the safe core (one-byte items).
fn apply_safe(s: &mut DArray<u8>, op: &Op) -> Out {
    match op {
        Op::PushBackArray(data) => Out::Index(s.push_back_array(data)),
        Op::Reserve(n) => {
            s.reserve(*n);
            Out::Nothing
        }
        Op::EnsureCapacity(n) => {
            s.ensure_capacity(*n);
            Out::Nothing
        }
        Op::Resize(n) => {
            s.resize(*n);
            Out::Nothing
        }
        Op::Clear => {
            s.clear();
            Out::Nothing
        }
        Op::Erase(i) => {
            s.erase(*i);
            Out::Nothing
        }
        Op::PopBack => {
            s.pop_back();
            Out::Nothing
        }
        Op::PushBack(item) => Out::Index(s.push_back(item[0])),
        Op::PushBackNew(_) => {
            assert_eq!(*s.push_back_new(), 0);
            Out::Item(s.len() - 1)
        }
        Op::Insert(i, item) => {
            s.insert(*i, item[0]);
            Out::Nothing
        }
        Op::EraseRange(a, b) => {
            s.erase_range(*a, *b);
            Out::Nothing
        }
        Op::PopFront => {
            s.pop_front();
            Out::Nothing
        }
        Op::Free => {
            s.free();
            Out::Nothing
        }
    }
}

/// The leading slots whose bytes the header determines after `op`, given
/// `known` before it (see the module docs).
fn next_known(known: usize, op: &Op, before: &darray, after: &darray) -> usize {
    let known = match op {
        Op::Free => 0,
        Op::Reserve(n) if *n > before.capacity => known.min(before.num),
        _ => known,
    };
    known.max(after.num)
}

fn bytes<'a>(d: &darray, slots: usize, es: usize) -> &'a [u8] {
    if slots == 0 || d.array.is_null() {
        &[]
    } else {
        // SAFETY: `slots <= capacity`, so the buffer holds `slots * es` bytes.
        unsafe { slice::from_raw_parts(d.array as *const u8, slots * es) }
    }
}

fn check(
    es: usize,
    known: usize,
    a: &darray,
    b: &darray,
    safe: Option<&DArray<u8>>,
) -> Result<(), TestCaseError> {
    prop_assert_eq!(a.num, b.num);
    prop_assert_eq!(a.capacity, b.capacity);
    prop_assert_eq!(a.array.is_null(), b.array.is_null());
    prop_assert!(known <= a.capacity);
    prop_assert_eq!(bytes(a, known, es), bytes(b, known, es));
    if let Some(s) = safe {
        prop_assert_eq!(s.len(), a.num);
        prop_assert_eq!(s.capacity(), a.capacity);
        prop_assert_eq!(s.as_slice(), bytes(a, a.num, es));
    }
    Ok(())
}

fn empty() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

fn snapshot(d: &darray) -> darray {
    darray {
        array: d.array,
        num: d.num,
        capacity: d.capacity,
    }
}

fn run_twins(es: usize, ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut a = empty();
    let mut b = OracleDarray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
    let b: *mut darray = (&mut b as *mut OracleDarray).cast();
    let mut safe = DArray::<u8>::new();
    let mut known = 0;

    for op in &ops {
        let Some(op) = resolve(op, a.num) else {
            continue;
        };
        let before = snapshot(&a);
        // SAFETY: both arrays are valid and only these functions touch
        // them; `resolve` keeps indices within what the header allows.
        let (ra, rb) = unsafe { (apply(&RUST, es, &mut a, &op), apply(&C, es, b, &op)) };
        prop_assert_eq!(&ra, &rb, "{:?}", op);
        if es == 1 {
            prop_assert_eq!(apply_safe(&mut safe, &op), rb, "{:?}", op);
        }
        known = next_known(known, &op, &before, &a);
        // SAFETY: `b` points to the live oracle array above.
        check(es, known, &a, unsafe { &*b }, (es == 1).then_some(&safe))?;
    }

    // SAFETY: both buffers are bmem blocks (or null).
    unsafe {
        r::darray_free(&mut a);
        c_free(b);
    }
    Ok(())
}

fn run_mixed(es: usize, ops: Vec<(Op, bool)>) -> Result<(), TestCaseError> {
    let mut m = empty();
    let mut reference = empty();
    let mut known = 0;

    for (op, rust) in &ops {
        let Some(op) = resolve(op, reference.num) else {
            continue;
        };
        let imp = if *rust { &RUST } else { &C };
        let before = snapshot(&reference);
        // SAFETY: both arrays are valid; each step hands `m` to one complete
        // implementation, and both use the same bmem allocator.
        let (rm, rr) = unsafe {
            (
                apply(imp, es, &mut m, &op),
                apply(&C, es, &mut reference, &op),
            )
        };
        prop_assert_eq!(rm, rr, "{:?} rust={}", op, rust);
        known = next_known(known, &op, &before, &reference);
        check(es, known, &m, &reference, None)?;
    }

    // SAFETY: as above.
    unsafe {
        r::darray_free(&mut m);
        c_free(&mut reference);
    }
    Ok(())
}

fn element_size() -> impl Strategy<Value = usize> {
    prop::sample::select(vec![1usize, 4, 12, 16])
}

proptest! {
    #[test]
    fn darray_ops_match_c(es in element_size(), ops in vec(op(), 0..64)) {
        run_twins(es, ops)?;
    }

    #[test]
    fn darray_mixed_in_place_matches_c(
        es in element_size(),
        ops in vec((op(), any::<bool>()), 0..64),
    ) {
        run_mixed(es, ops)?;
    }
}

/// A 16-byte record like `struct encoder_packet_time`.
fn record(pts: i64, track: u32) -> [u8; 16] {
    let mut rec = [0u8; 16];
    rec[..8].copy_from_slice(&pts.to_ne_bytes());
    rec[8..12].copy_from_slice(&track.to_ne_bytes());
    rec
}

/// `test_darray_ops`'s resort and packet-time patterns as op lists, run
/// through the Rust and C functions in turn on one array, against the C
/// alone.
#[test]
fn interleaver_patterns_match_c() {
    // Packet times: push back new, pop the oldest, erase from the middle,
    // insert in sorted position, and discard a prefix (discard_to_idx).
    let mut ops = Vec::new();
    for round in 0usize..64 {
        let side = round % 3 != 0;
        ops.push((Op::PushBackNew(round.is_multiple_of(2)), side));
        ops.push((Op::Insert(round * 7, record(round as i64, 1)), !side));
        if round % 4 == 3 {
            ops.push((Op::PopFront, side));
        }
        if round % 5 == 4 {
            ops.push((Op::Erase(round), !side));
        }
        if round % 9 == 8 {
            ops.push((Op::EraseRange(0, 3), side));
        }
    }
    run_mixed(16, ops).unwrap();

    // The resort: every item into a zeroed array, then the old one freed.
    let mut ops = Vec::new();
    for (i, pts) in [30i64, 10, 50, 20, 40, 0, 60].into_iter().enumerate() {
        ops.push((Op::Insert(i % 3, record(pts, i as u32)), i % 2 == 0));
    }
    ops.push((Op::Free, true));
    for (i, pts) in [5i64, 15, 25].into_iter().enumerate() {
        ops.push((Op::Insert(0, record(pts, i as u32)), i % 2 == 1));
    }
    run_mixed(16, ops).unwrap();
}

/// The bmem accounting of a hand-off: C allocates and Rust grows and frees,
/// Rust allocates and C grows and frees, and removal on either side never
/// allocates or frees.
fn measure_allocs() {
    let es = 16;
    let rec = record(1, 2);
    // SAFETY: plain counter reads and arrays that only these functions touch.
    unsafe {
        let base = bnum_allocs();
        let mut d = empty();

        c_insert(es, &mut d, 0, rec.as_ptr().cast());
        assert_eq!(bnum_allocs(), base + 1);
        for i in 0..20 {
            r::darray_insert(es, &mut d, i / 2, rec.as_ptr().cast());
            c_push_back_new(es, &mut d, i % 2 == 0);
            r::darray_push_back_new(es, &mut d);
        }
        assert_eq!(d.capacity, 64);
        assert_eq!(bnum_allocs(), base + 1);
        r::darray_erase_range(es, &mut d, 0, 10);
        c_pop_front(es, &mut d);
        r::darray_pop_front(es, &mut d);
        c_erase_range(es, &mut d, 0, d.num);
        assert!(!d.array.is_null());
        assert_eq!(bnum_allocs(), base + 1);
        r::darray_free(&mut d);
        assert_eq!(bnum_allocs(), base);

        r::darray_push_back_new(es, &mut d);
        r::darray_insert(es, &mut d, 0, rec.as_ptr().cast());
        assert_eq!(bnum_allocs(), base + 1);
        c_insert(es, &mut d, 1, rec.as_ptr().cast());
        assert_eq!(d.capacity, 4);
        assert_eq!(bnum_allocs(), base + 1);
        c_free(&mut d);
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
