//! Exact parity for a by-value snapshot; no argument-table dereferences.
use core::ffi::{c_char, c_int};
use obs_core::ffi::core_cmdline_getter::{CommandLineArgs, obs_get_cmdline_args};
use proptest::prelude::*;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_core_cmdline_set(argc: c_int, argv: *mut *mut c_char);
    fn oracle_core_cmdline_layout(layout: *mut usize);
    fn oracle_obs_get_cmdline_args() -> CommandLineArgs;
}

#[test]
fn real_public_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the oracle writes exactly eight size_t entries.
    unsafe { oracle_core_cmdline_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<CommandLineArgs>(),
            align_of::<CommandLineArgs>(),
            std::mem::offset_of!(CommandLineArgs, argc),
            std::mem::offset_of!(CommandLineArgs, argv),
            size_of::<c_int>(),
            align_of::<c_int>(),
            size_of::<*mut *mut c_char>(),
            align_of::<*mut *mut c_char>(),
        ]
    );
}

fn compare(argc: i32, address: usize) {
    let argv = std::ptr::without_provenance_mut::<*mut c_char>(address);
    // SAFETY: LOCK protects the oracle-owned static. Both getters copy the
    // count and pointer only, without dereferencing the generated pointer.
    unsafe {
        oracle_core_cmdline_set(argc, argv);
        let expected = oracle_obs_get_cmdline_args();
        let mut actual = obs_get_cmdline_args();
        assert_eq!((actual.argc, actual.argv), (expected.argc, expected.argv));
        assert_eq!((actual.argc, actual.argv), (argc, argv));
        actual.argc = argc.wrapping_add(1);
        actual.argv = std::ptr::null_mut();
        assert_eq!(actual.argc, argc.wrapping_add(1));
        assert!(actual.argv.is_null());
        let again = obs_get_cmdline_args();
        assert_eq!((again.argc, again.argv), (argc, argv));
        let unchanged = oracle_obs_get_cmdline_args();
        assert_eq!((unchanged.argc, unchanged.argv), (argc, argv));
        oracle_core_cmdline_set(0, std::ptr::null_mut());
    }
}

#[test]
fn characterized_edges() {
    let _guard = LOCK.lock().unwrap();
    for argc in [i32::MIN, -1, 0, 1, 4, i32::MAX] {
        for address in [0, 1, usize::MAX / 2, usize::MAX] {
            compare(argc, address);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn snapshot_matches_c(argc in any::<i32>(), address in any::<usize>()) {
        let _guard = LOCK.lock().unwrap();
        compare(argc, address);
    }
}
