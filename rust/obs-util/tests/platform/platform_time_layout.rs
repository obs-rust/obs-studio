//! Tier 2: `struct timespec` in the `os_nstime_to_timespec` shim matches the
//! C header.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::platform_time as c;
use obs_util::ffi::platform_time::timespec;

#[test]
fn timespec_layout_matches_c_header() {
    // SAFETY: the oracle layout functions take no arguments.
    unsafe {
        assert_eq!(size_of::<timespec>(), c::oracle_timespec_size());
        assert_eq!(align_of::<timespec>(), c::oracle_timespec_align());
        assert_eq!(
            offset_of!(timespec, tv_sec),
            c::oracle_timespec_offset_tv_sec()
        );
        assert_eq!(
            offset_of!(timespec, tv_nsec),
            c::oracle_timespec_offset_tv_nsec()
        );
    }
}
