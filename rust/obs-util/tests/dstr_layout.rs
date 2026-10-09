//! Tier 2: `#[repr(C)] dstr` matches the layout the C compiler produces for
//! `libobs/util/dstr.h`. `struct dstr` is the contract for the header-inline
//! helpers (`dstr_init`, `dstr_free`, `dstr_ensure_capacity`, ...), which stay
//! in C and read and write its fields directly.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::dstr as c;
use obs_util::ffi::dstr::dstr;

#[test]
fn dstr_layout_matches_c_header() {
    // SAFETY: the oracle layout functions take no arguments and only return constants.
    unsafe {
        assert_eq!(size_of::<dstr>(), c::oracle_sizeof_dstr());
        assert_eq!(align_of::<dstr>(), c::oracle_alignof_dstr());
        assert_eq!(offset_of!(dstr, array), c::oracle_offsetof_dstr_array());
        assert_eq!(offset_of!(dstr, len), c::oracle_offsetof_dstr_len());
        assert_eq!(
            offset_of!(dstr, capacity),
            c::oracle_offsetof_dstr_capacity()
        );
    }
}
