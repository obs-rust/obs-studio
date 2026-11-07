//! Exact parity: no intended differences. Null `info` is undefined in C and
//! excluded. Counts deliberately need not agree with the table length.
use obs_core::ffi::module_failure_info::{ModuleFailureInfo, obs_module_failure_info_free};
use obs_util::ffi::darray::bmalloc;
use proptest::prelude::*;
use std::ffi::c_char;
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_obs_module_failure_info_free(info: *mut ModuleFailureInfo);
    fn oracle_module_failure_info_size() -> usize;
    fn oracle_module_failure_info_align() -> usize;
    fn oracle_module_failure_info_table_offset() -> usize;
    fn oracle_module_failure_info_count_offset() -> usize;
}

#[test]
fn real_header_layout() {
    // SAFETY: these C helpers take no arguments and measure the real header.
    unsafe {
        assert_eq!(
            size_of::<ModuleFailureInfo>(),
            oracle_module_failure_info_size()
        );
        assert_eq!(
            align_of::<ModuleFailureInfo>(),
            oracle_module_failure_info_align()
        );
        assert_eq!(
            offset_of!(ModuleFailureInfo, failed_modules),
            oracle_module_failure_info_table_offset()
        );
        assert_eq!(
            offset_of!(ModuleFailureInfo, count),
            oracle_module_failure_info_count_offset()
        );
    }
}

fn compare(count: usize, allocated: bool, repeat: usize) {
    for free in [
        obs_module_failure_info_free,
        oracle_obs_module_failure_info_free,
    ] {
        let borrowed = c"module-name";
        // SAFETY: allocate one pointer slot, initialize it, then transfer only
        // the table to cleanup. The borrowed static string must never be freed.
        let table = unsafe {
            if allocated {
                let table = bmalloc(size_of::<*mut c_char>()).cast::<*mut c_char>();
                table.write(borrowed.as_ptr().cast_mut());
                table
            } else {
                std::ptr::null_mut()
            }
        };
        let mut info = ModuleFailureInfo {
            failed_modules: table,
            count,
        };
        for _ in 0..repeat {
            // SAFETY: live writable struct with a null or owned table.
            unsafe { free(&raw mut info) };
            assert!(info.failed_modules.is_null());
            assert_eq!(info.count, count);
            assert_eq!(borrowed.to_bytes(), b"module-name");
        }
    }
}

#[test]
fn characterized_edges() {
    for count in [0, 1, 3, usize::MAX] {
        compare(count, false, 2);
        compare(count, true, 2);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        .. ProptestConfig::default()
    })]
    #[test]
    fn matches_c(count in any::<usize>(), allocated in any::<bool>(), repeat in 1usize..5) {
        compare(count, allocated, repeat);
    }
}
