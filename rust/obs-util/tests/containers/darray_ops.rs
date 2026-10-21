//! Tier 1: the `darray.h` operations the output interleaver uses (#1594):
//! insert, erase range, pop front and push back new. Each test named after a
//! case mirrors that case of `test/cmocka/test_darray_ops.c` 1:1, through the
//! raw functions in [`obs_util::ffi::darray`] on a `struct darray` with bmem
//! memory, as the C test runs the header. The `safe_*` tests are the same
//! operations on the safe [`DArray`]; the rest are new edge cases.
//!
//! The `bnum_allocs()` checks (`alloc_accounting`, `resort_pattern`) read
//! the process-wide bmem counter, so they run in their own process
//! (`common::isolated`, #1133).
//!
//! `removal_leaves_tail_bytes` keeps the C case "characterized, not
//! endorsed": removal moves the tail down and lowers `num`, and the slots
//! past the new `num` keep their old values.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::{align_of, offset_of, size_of};
use core::ptr;

use obs_c_oracle as _;
use obs_util::darray::DArray;
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::darray::{self as r, darray};

/// Shaped like the interleaver's `struct encoder_packet_time`: more than one
/// word, so the moves are checked on element size, not on bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Record {
    pts: i64,
    track: u32,
    tag: u32,
}

/// `DARRAY(T)`: a `struct darray` of `T` items, driven only through the raw
/// functions with `element_size = size_of::<T>()`.
struct Da<T: Copy> {
    da: darray,
    _items: PhantomData<T>,
}

impl<T: Copy> Da<T> {
    /// `da_init`, or the `memset` to zero that `resort_interleaved_packets`
    /// uses: the same state.
    fn new() -> Self {
        Self {
            da: darray {
                array: ptr::null_mut(),
                num: 0,
                capacity: 0,
            },
            _items: PhantomData,
        }
    }

    const ES: usize = size_of::<T>();

    fn num(&self) -> usize {
        self.da.num
    }

    fn capacity(&self) -> usize {
        self.da.capacity
    }

    fn buffer(&self) -> *mut c_void {
        self.da.array
    }

    fn items(&self) -> Vec<T> {
        (0..self.da.num).map(|i| self.slot(i)).collect()
    }

    /// Item `i` of the buffer, which may be past `num` (C: `v.array[i]`).
    fn slot(&self, i: usize) -> T {
        assert!(i < self.da.capacity, "slot {i} past the capacity");
        // SAFETY: the buffer holds `capacity` items; the tests read only
        // slots they wrote.
        unsafe { (self.da.array as *const T).add(i).read() }
    }

    fn slot_ptr(&self, i: usize) -> *mut c_void {
        // SAFETY: only compared, never dereferenced, and within the buffer.
        unsafe { (self.da.array as *mut T).add(i).cast() }
    }

    fn reserve(&mut self, n: usize) {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_reserve(Self::ES, &mut self.da, n) }
    }

    fn ensure_capacity(&mut self, n: usize) {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_ensure_capacity(Self::ES, &mut self.da, n) }
    }

    fn push_back(&mut self, item: T) -> usize {
        // SAFETY: a valid darray of `T` items and one readable item.
        unsafe { r::darray_push_back(Self::ES, &mut self.da, (&item as *const T).cast()) }
    }

    fn push_back_new(&mut self) -> *mut T {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_push_back_new(Self::ES, &mut self.da).cast() }
    }

    fn insert(&mut self, idx: usize, item: T) {
        // SAFETY: a valid darray of `T` items and one readable item.
        unsafe { r::darray_insert(Self::ES, &mut self.da, idx, (&item as *const T).cast()) }
    }

    fn erase(&mut self, idx: usize) {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_erase(Self::ES, &mut self.da, idx) }
    }

    fn erase_range(&mut self, start: usize, end: usize) {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_erase_range(Self::ES, &mut self.da, start, end) }
    }

    fn pop_front(&mut self) {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_pop_front(Self::ES, &mut self.da) }
    }

    fn pop_back(&mut self) {
        // SAFETY: a valid darray of `T` items.
        unsafe { r::darray_pop_back(Self::ES, &mut self.da) }
    }

    /// `da_free`.
    fn free(&mut self) {
        // SAFETY: the buffer is null or a bmem block.
        unsafe { r::darray_free(&mut self.da) }
    }
}

impl<T: Copy> Drop for Da<T> {
    fn drop(&mut self) {
        self.free();
    }
}

/// `bnum_allocs()`.
fn allocs_now() -> core::ffi::c_long {
    // SAFETY: a plain read of the bmem counter.
    unsafe { bnum_allocs() }
}

/// `fill_ints`: appends `first, first + 1, ...` with `darray_push_back`.
fn fill_ints(v: &mut Da<i32>, first: i32, count: i32) {
    for i in 0..count {
        v.push_back(first + i);
    }
}

#[test]
fn layout() {
    // `test_layout` without the DARRAY(type) union aliasing, which has no
    // Rust counterpart: the raw functions always take a `struct darray`.
    #[repr(C)]
    struct AlignProbe {
        c: u8,
        da: darray,
    }
    assert_eq!(size_of::<*mut c_void>(), size_of::<usize>());
    assert_eq!(size_of::<darray>(), 3 * size_of::<usize>());
    assert_eq!(offset_of!(AlignProbe, da), size_of::<usize>());
    assert_eq!(align_of::<darray>(), align_of::<usize>());
    assert_eq!(offset_of!(darray, array), 0);
    assert_eq!(offset_of!(darray, num), size_of::<usize>());
    assert_eq!(offset_of!(darray, capacity), 2 * size_of::<usize>());
}

#[test]
fn insert_into_empty() {
    let mut v = Da::<i32>::new();

    // idx == num takes the push_back path; the first allocation is 1.
    v.insert(0, 7);
    assert!(!v.buffer().is_null());
    assert_eq!(v.capacity(), 1);
    assert_eq!(v.items(), [7]);

    v.free();
}

#[test]
fn insert_front_middle_end() {
    let mut v = Da::<i32>::new();
    v.reserve(8);
    fill_ints(&mut v, 1, 4);
    assert_eq!(v.items(), [1, 2, 3, 4]);

    v.insert(0, 10);
    assert_eq!(v.items(), [10, 1, 2, 3, 4]);
    v.insert(3, 20);
    assert_eq!(v.items(), [10, 1, 2, 20, 3, 4]);
    v.insert(v.num(), 30);
    assert_eq!(v.items(), [10, 1, 2, 20, 3, 4, 30]);
    v.insert(v.num() - 1, 10);
    assert_eq!(v.items(), [10, 1, 2, 20, 3, 4, 10, 30]);

    // No growth while the reserved capacity holds.
    assert_eq!(v.capacity(), 8);
}

#[test]
fn insert_growth() {
    let mut v = Da::<i32>::new();

    // Inserting at the front each time: capacity 1, 2, 4, 4, 8.
    let caps = [1, 2, 4, 4, 8, 8, 8, 8, 16];
    for (i, &cap) in caps.iter().enumerate() {
        v.insert(0, i as i32);
        assert_eq!(v.num(), i + 1);
        assert_eq!(v.capacity(), cap);
    }
    assert_eq!(v.items(), [8, 7, 6, 5, 4, 3, 2, 1, 0]);

    // A middle insert into a full array grows it and keeps every item.
    v.free();
    fill_ints(&mut v, 1, 4);
    assert_eq!(v.capacity(), 4);
    v.insert(2, 99);
    assert_eq!(v.capacity(), 8);
    assert_eq!(v.items(), [1, 2, 99, 3, 4]);

    // After a reserve that is not a power of two, growth doubles it.
    v.free();
    v.reserve(3);
    fill_ints(&mut v, 1, 3);
    v.insert(1, 99);
    assert_eq!(v.capacity(), 6);
    assert_eq!(v.items(), [1, 99, 2, 3]);
}

#[test]
fn insert_records() {
    let mut v = Da::<Record>::new();

    // Sorted insertion by pts, as insert_interleaved_packet does.
    let order = [50i64, 10, 40, 20, 30, 60, 0];
    for (i, &pts) in order.iter().enumerate() {
        let rec = Record {
            pts,
            track: i as u32,
            tag: 0xA5A5_A5A5,
        };
        let mut idx = 0;
        while idx < v.num() && v.slot(idx).pts <= rec.pts {
            idx += 1;
        }
        v.insert(idx, rec);
    }

    assert_eq!(v.num(), 7);
    assert_eq!(v.capacity(), 8);
    for (i, rec) in v.items().iter().enumerate() {
        assert_eq!(rec.pts, i as i64 * 10);
        assert_eq!(rec.tag, 0xA5A5_A5A5);
    }
    assert_eq!(v.slot(0).track, 6);
    assert_eq!(v.slot(5).track, 0);
    assert_eq!(v.slot(6).track, 5);
}

#[test]
fn erase() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 5);
    let buffer = v.buffer();

    v.erase(2);
    assert_eq!(v.items(), [1, 2, 4, 5]);
    v.erase(0);
    assert_eq!(v.items(), [2, 4, 5]);
    v.erase(v.num() - 1);
    assert_eq!(v.items(), [2, 4]);
    v.erase(1);
    v.erase(0);
    assert_eq!(v.num(), 0);

    // Erasing never shrinks or frees the buffer.
    assert_eq!(v.buffer(), buffer);
    assert_eq!(v.capacity(), 8);
}

#[test]
fn erase_range() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 8);
    let buffer = v.buffer();

    // From the front, as discard_to_idx does.
    v.erase_range(0, 3);
    assert_eq!(v.items(), [4, 5, 6, 7, 8]);

    // From the middle.
    v.erase_range(1, 3);
    assert_eq!(v.items(), [4, 7, 8]);

    // A range of one is darray_erase.
    v.erase_range(1, 2);
    assert_eq!(v.items(), [4, 8]);

    // At the end: nothing moves.
    fill_ints(&mut v, 20, 3);
    assert_eq!(v.items(), [4, 8, 20, 21, 22]);
    v.erase_range(3, 5);
    assert_eq!(v.items(), [4, 8, 20]);

    // The whole array: only num changes.
    v.erase_range(0, v.num());
    assert_eq!(v.num(), 0);
    assert_eq!(v.buffer(), buffer);
    assert_eq!(v.capacity(), 8);

    // A whole array of one is darray_erase too.
    fill_ints(&mut v, 5, 1);
    v.erase_range(0, 1);
    assert_eq!(v.num(), 0);
}

#[test]
fn erase_range_records() {
    let mut v = Da::<Record>::new();
    for i in 0u32..6 {
        v.push_back(Record {
            pts: i64::from(i) * 100,
            track: i,
            tag: !i,
        });
    }

    v.erase_range(0, 4);
    assert_eq!(v.num(), 2);
    assert_eq!(
        v.slot(0),
        Record {
            pts: 400,
            track: 4,
            tag: !4
        }
    );
    assert_eq!(
        v.slot(1),
        Record {
            pts: 500,
            track: 5,
            tag: !5
        }
    );
}

#[test]
fn removal_leaves_tail_bytes() {
    // Characterized, not endorsed: removal moves the tail down and lowers
    // num, but the slots past the new num keep their old values. Nothing
    // may read them; this pins that removal does not clear them.
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 5);

    v.erase(1);
    assert_eq!(v.items(), [1, 3, 4, 5]);
    assert_eq!(v.slot(4), 5);

    v.erase_range(0, 2);
    assert_eq!(v.items(), [4, 5]);
    assert_eq!(v.slot(2), 4);
    assert_eq!(v.slot(3), 5);

    v.pop_front();
    assert_eq!(v.items(), [5]);
    assert_eq!(v.slot(1), 5);

    // Removing everything moves nothing.
    v.erase_range(0, 1);
    assert_eq!(v.num(), 0);
    assert_eq!(v.slot(0), 5);
}

#[test]
fn pop_front() {
    let mut v = Da::<Record>::new();
    for i in 0u32..3 {
        v.push_back(Record {
            pts: i64::from(i),
            track: i,
            tag: i + 1,
        });
    }
    let buffer = v.buffer();

    v.pop_front();
    assert_eq!(v.num(), 2);
    assert_eq!(v.slot(0).pts, 1);
    assert_eq!(v.slot(1).pts, 2);
    assert_eq!(v.slot(1).tag, 3);

    v.pop_front();
    v.pop_front();
    assert_eq!(v.num(), 0);
    assert_eq!(v.buffer(), buffer);
    assert_eq!(v.capacity(), 4);
}

#[test]
fn push_back_new() {
    let mut v = Da::<Record>::new();

    // Each call returns the new last item, zeroed, after any growth.
    let caps = [1, 2, 4, 4, 8];
    for (i, &cap) in caps.iter().enumerate() {
        let p = v.push_back_new();
        assert_eq!(v.num(), i + 1);
        assert_eq!(v.capacity(), cap);
        assert_eq!(p.cast(), v.slot_ptr(i));
        // SAFETY: `p` is item `i` of the live buffer.
        unsafe {
            assert_eq!(*p, Record::default());
            (*p).pts = i as i64;
            (*p).tag = 0xFFFF_FFFF;
        }
    }
    for (i, rec) in v.items().iter().enumerate() {
        assert_eq!(rec.pts, i as i64);
    }

    // A reused slot is zeroed too, not left with its old bytes.
    v.pop_back();
    let p = v.push_back_new();
    assert_eq!(p.cast(), v.slot_ptr(4));
    assert_eq!(v.slot(4), Record::default());

    // C has the da_push_back_new macro (GCC and Clang) and the
    // darray_push_back_new function behind it elsewhere; both do the same
    // steps, which the one Rust function is. The C test's second half:
    // SAFETY: `p` is item 4 of the live buffer.
    unsafe { (*p).tag = 0xFFFF_FFFF };
    v.pop_back();
    let p = v.push_back_new();
    assert_eq!(p.cast(), v.slot_ptr(4));
    assert_eq!(v.num(), 5);
    assert_eq!(v.slot(4).tag, 0);
    let p = v.push_back_new();
    assert_eq!(p.cast(), v.slot_ptr(5));
    assert_eq!(v.num(), 6);
    assert_eq!(v.capacity(), 8);
    assert_eq!(v.slot(5).pts, 0);
}

fn measure_alloc_accounting() {
    let allocs = allocs_now();
    let mut v = Da::<Record>::new();
    let rec = Record {
        pts: 1,
        track: 2,
        tag: 3,
    };

    // The first insert allocates; growth replaces the block, one at a time.
    v.insert(0, rec);
    assert_eq!(allocs_now(), allocs + 1);
    for _ in 0..20 {
        v.insert(v.num() / 2, rec);
    }
    assert_eq!(v.capacity(), 32);
    assert_eq!(allocs_now(), allocs + 1);
    for _ in 0..20 {
        v.push_back_new();
    }
    assert_eq!(v.capacity(), 64);
    assert_eq!(allocs_now(), allocs + 1);

    // Removal never allocates or frees.
    v.erase(3);
    v.erase_range(0, 10);
    v.pop_front();
    v.erase_range(0, v.num());
    assert!(!v.buffer().is_null());
    assert_eq!(allocs_now(), allocs + 1);

    v.free();
    assert!(v.buffer().is_null());
    assert_eq!(v.num(), 0);
    assert_eq!(v.capacity(), 0);
    assert_eq!(allocs_now(), allocs);

    // Freeing an array that never allocated is fine.
    v.free();
    assert_eq!(allocs_now(), allocs);
}

const ALLOC_ACCOUNTING: &str = concat!(module_path!(), "::alloc_accounting");

#[test]
fn alloc_accounting() {
    if crate::common::isolated(ALLOC_ACCOUNTING) {
        measure_alloc_accounting();
    }
}

fn measure_resort_pattern() {
    // resort_interleaved_packets: take the darray out of its owner, zero
    // the owner, insert every old item back in sorted position, then free
    // the old buffer.
    let allocs = allocs_now();
    let mut owned = Da::<Record>::new();
    for (i, &pts) in [30i64, 10, 50, 20, 40].iter().enumerate() {
        owned.push_back(Record {
            pts,
            track: i as u32,
            tag: 0,
        });
    }

    let mut old = Da::<Record>::new();
    old.da = core::mem::replace(
        &mut owned.da,
        darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        },
    );
    for i in 0..old.num() {
        let rec = old.slot(i);
        let mut idx = 0;
        while idx < owned.num() && owned.slot(idx).pts < rec.pts {
            idx += 1;
        }
        owned.insert(idx, rec);
    }
    assert_eq!(allocs_now(), allocs + 2);
    old.free();
    assert_eq!(allocs_now(), allocs + 1);

    assert_eq!(owned.num(), 5);
    assert_eq!(owned.capacity(), 8);
    for (i, rec) in owned.items().iter().enumerate() {
        assert_eq!(rec.pts, (i as i64 + 1) * 10);
    }

    owned.free();
    assert_eq!(allocs_now(), allocs);
}

const RESORT_PATTERN: &str = concat!(module_path!(), "::resort_pattern");

#[test]
fn resort_pattern() {
    if crate::common::isolated(RESORT_PATTERN) {
        measure_resort_pattern();
    }
}

#[test]
fn packet_time_pattern() {
    // The encoder_packet_times arrays: append zeroed records with
    // push_back_new, drop the oldest with pop_front, and erase a matching
    // one from the middle with erase.
    let mut v = Da::<Record>::new();

    for round in 0i64..40 {
        let p = v.push_back_new();
        // SAFETY: `p` is the new last item of the live buffer.
        unsafe {
            (*p).pts = round;
            (*p).track = (round % 3) as u32;
        }

        if round % 4 == 3 {
            v.pop_front();
        }
        if round % 5 == 4
            && let Some(i) = (0..v.num()).find(|&i| v.slot(i).pts == round - 2)
        {
            v.erase(i);
        }
    }

    // 40 pushes, 10 pops from the front and 8 matched erases.
    let expected = [
        13i64, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25, 26, 28, 29, 30, 31, 33, 34, 35, 36, 38, 39,
    ];
    assert_eq!(v.num(), expected.len());
    for (rec, &pts) in v.items().iter().zip(&expected) {
        assert_eq!(rec.pts, pts);
        assert_eq!(rec.track, (pts % 3) as u32);
    }
    assert_eq!(v.capacity(), 32);
}

// New edge cases.

#[test]
fn insert_at_num_into_full_array_grows_by_push_back() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 4);
    assert_eq!(v.capacity(), 4);
    v.insert(4, 5);
    assert_eq!(v.capacity(), 8);
    assert_eq!(v.items(), [1, 2, 3, 4, 5]);
}

#[test]
fn insert_into_reserved_empty_array_keeps_buffer() {
    let mut v = Da::<i32>::new();
    v.reserve(4);
    let buffer = v.buffer();
    v.insert(0, 1);
    v.insert(0, 2);
    assert_eq!(v.items(), [2, 1]);
    assert_eq!(v.buffer(), buffer);
    assert_eq!(v.capacity(), 4);
}

#[test]
fn insert_moves_whole_items_of_odd_size() {
    let mut v = Da::<[u8; 3]>::new();
    v.push_back([1, 2, 3]);
    v.push_back([4, 5, 6]);
    v.insert(1, [7, 8, 9]);
    v.insert(0, [10, 11, 12]);
    assert_eq!(v.items(), [[10, 11, 12], [1, 2, 3], [7, 8, 9], [4, 5, 6]]);
}

#[test]
fn growth_copies_the_stale_tail() {
    // darray_ensure_capacity copies the whole old capacity, so the slots
    // past num that removal left behind move into the new buffer too.
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 4);
    v.erase_range(0, 2);
    assert_eq!(v.items(), [3, 4]);
    let old = v.buffer();
    v.ensure_capacity(5);
    assert_ne!(v.buffer(), old);
    assert_eq!(v.capacity(), 8);
    assert_eq!(v.items(), [3, 4]);
    assert_eq!(v.slot(2), 3);
    assert_eq!(v.slot(3), 4);
    v.push_back_new();
    assert_eq!(v.items(), [3, 4, 0]);
    assert_eq!(v.slot(3), 4);
}

#[test]
fn push_back_new_zeroes_only_the_new_item() {
    let mut v = Da::<[u8; 3]>::new();
    v.push_back([1, 2, 3]);
    v.push_back([4, 5, 6]);
    v.push_back([7, 8, 9]);
    v.erase_range(1, 3);
    let p = v.push_back_new();
    assert_eq!(p.cast(), v.slot_ptr(1));
    assert_eq!(v.slot(1), [0, 0, 0]);
    // Slot 2 still holds what removal left there.
    assert_eq!(v.slot(2), [7, 8, 9]);
}

#[test]
fn push_back_new_on_zeroed_array_allocates_one() {
    let mut v = Da::<Record>::new();
    let p = v.push_back_new();
    assert!(!p.is_null());
    assert_eq!(v.num(), 1);
    assert_eq!(v.capacity(), 1);
    assert_eq!(v.slot(0), Record::default());
}

#[test]
fn pop_front_on_empty_is_a_no_op() {
    // C asserts; the body then checks num, so an NDEBUG build does nothing.
    let mut v = Da::<i32>::new();
    v.pop_front();
    assert!(v.buffer().is_null());
    assert_eq!((v.num(), v.capacity()), (0, 0));
    fill_ints(&mut v, 1, 1);
    v.pop_front();
    v.pop_front();
    assert_eq!(v.num(), 0);
    assert_eq!(v.slot(0), 1);
}

#[test]
fn erase_range_at_the_front_leaves_one() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 3);
    v.erase_range(0, 2);
    assert_eq!(v.items(), [3]);
    assert_eq!(v.slot(1), 2);
    assert_eq!(v.slot(2), 3);
}

#[test]
#[should_panic(expected = "darray_insert: idx > num")]
fn insert_past_num_panics() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 2);
    v.insert(3, 0);
}

#[test]
#[should_panic(expected = "darray_erase_range: end <= start")]
fn erase_range_empty_panics() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 3);
    v.erase_range(1, 1);
}

#[test]
#[should_panic(expected = "darray_erase_range: end > num")]
fn erase_range_past_num_panics() {
    let mut v = Da::<i32>::new();
    fill_ints(&mut v, 1, 3);
    v.erase_range(1, 4);
}

// The same operations on the safe core.

#[test]
fn safe_insert() {
    let mut da: DArray<i32> = DArray::new();
    let caps = [1, 2, 4, 4, 8];
    for (i, &cap) in caps.iter().enumerate() {
        da.insert(0, i as i32);
        assert_eq!(da.capacity(), cap);
    }
    assert_eq!(da.as_slice(), [4, 3, 2, 1, 0]);
    da.insert(2, 9);
    da.insert(da.len(), 7);
    assert_eq!(da.as_slice(), [4, 3, 9, 2, 1, 0, 7]);
    assert_eq!(da.capacity(), 8);

    let mut da: DArray<i32> = DArray::new();
    da.reserve(3);
    da.push_back_array(&[1, 2, 3]);
    da.insert(1, 99);
    assert_eq!(da.capacity(), 6);
    assert_eq!(da.as_slice(), [1, 99, 2, 3]);
}

#[test]
fn safe_erase_range() {
    let mut da: DArray<i32> = DArray::new();
    da.push_back_array(&[1, 2, 3, 4, 5, 6, 7, 8]);
    da.erase_range(0, 3);
    assert_eq!(da.as_slice(), [4, 5, 6, 7, 8]);
    da.erase_range(1, 3);
    assert_eq!(da.as_slice(), [4, 7, 8]);
    da.erase_range(1, 2);
    assert_eq!(da.as_slice(), [4, 8]);
    da.erase_range(0, 2);
    assert!(da.is_empty());
    assert_eq!(da.capacity(), 8);
}

#[test]
fn safe_pop_front() {
    let mut da: DArray<i32> = DArray::new();
    da.pop_front();
    assert_eq!(da.capacity(), 0);
    da.push_back_array(&[1, 2, 3]);
    da.pop_front();
    assert_eq!(da.as_slice(), [2, 3]);
    da.pop_front();
    da.pop_front();
    da.pop_front();
    assert!(da.is_empty());
    assert_eq!(da.capacity(), 3);
}

#[test]
fn safe_push_back_new() {
    let mut da: DArray<Record> = DArray::new();
    let caps = [1, 2, 4, 4, 8];
    for (i, &cap) in caps.iter().enumerate() {
        let rec = da.push_back_new();
        assert_eq!(*rec, Record::default());
        rec.pts = i as i64;
        assert_eq!(da.capacity(), cap);
    }
    da.pop_back();
    assert_eq!(*da.push_back_new(), Record::default());
    assert_eq!(da.len(), 5);
    assert_eq!(da.as_slice()[3].pts, 3);
}

#[test]
#[should_panic(expected = "darray_insert: idx > num")]
fn safe_insert_past_len_panics() {
    let mut da: DArray<i32> = DArray::new();
    da.insert(1, 0);
}

#[test]
#[should_panic(expected = "darray_erase_range: end <= start")]
fn safe_erase_range_reversed_panics() {
    let mut da: DArray<i32> = DArray::new();
    da.push_back_array(&[1, 2, 3]);
    da.erase_range(2, 1);
}
