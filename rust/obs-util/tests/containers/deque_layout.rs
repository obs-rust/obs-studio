//! Tier 2: `#[repr(C)] deque` matches the layout the C compiler produces for
//! `libobs/util/deque.h`. `deque_layout_matches_c_header` also mirrors
//! `test_layout` in `test/cmocka/test_deque.c`.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::deque as c;
use obs_util::ffi::deque::deque;

#[test]
fn deque_layout_matches_c_header() {
    // SAFETY: the oracle layout functions take no arguments and only return constants.
    unsafe {
        assert_eq!(size_of::<deque>(), c::oracle_deque_size());
        assert_eq!(align_of::<deque>(), c::oracle_deque_align());
        assert_eq!(offset_of!(deque, data), c::oracle_deque_offset_data());
        assert_eq!(offset_of!(deque, size), c::oracle_deque_offset_size());
        assert_eq!(
            offset_of!(deque, start_pos),
            c::oracle_deque_offset_start_pos()
        );
        assert_eq!(offset_of!(deque, end_pos), c::oracle_deque_offset_end_pos());
        assert_eq!(
            offset_of!(deque, capacity),
            c::oracle_deque_offset_capacity()
        );
    }
}

/// `test_layout`: five pointer-sized fields in declaration order.
#[test]
fn layout() {
    let w = size_of::<usize>();
    assert_eq!(size_of::<*mut u8>(), w);
    assert_eq!(size_of::<deque>(), 5 * w);
    assert_eq!(align_of::<deque>(), w);
    assert_eq!(offset_of!(deque, data), 0);
    assert_eq!(offset_of!(deque, size), w);
    assert_eq!(offset_of!(deque, start_pos), 2 * w);
    assert_eq!(offset_of!(deque, end_pos), 3 * w);
    assert_eq!(offset_of!(deque, capacity), 4 * w);
}
