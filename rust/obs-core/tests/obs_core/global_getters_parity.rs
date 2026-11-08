//! Tier 3: exact pointer identity, including nulls and independent field changes.
//! Only this test touches the test-owned oracle core; other category modules do
//! not use these getters. No pointee is dereferenced, retained, or freed.
use obs_core::ffi::global_getters::{obs_get_locale, obs_get_proc_handler, obs_get_signal_handler};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn oracle_core_global_set(locale: *mut c_char, signals: *mut c_void, procs: *mut c_void);
    fn oracle_obs_get_locale() -> *const c_char;
    fn oracle_obs_get_signal_handler() -> *mut c_void;
    fn oracle_obs_get_proc_handler() -> *mut c_void;
}

fn compare(fields: [usize; 3]) {
    // SAFETY: both implementations read only live pointer slots owned by the
    // oracle fixture. The arbitrary address tokens are never dereferenced.
    unsafe {
        oracle_core_global_set(
            std::ptr::without_provenance_mut(fields[0]),
            std::ptr::without_provenance_mut(fields[1]),
            std::ptr::without_provenance_mut(fields[2]),
        );
        for _ in 0..2 {
            assert_eq!(obs_get_locale(), oracle_obs_get_locale());
            assert_eq!(obs_get_signal_handler(), oracle_obs_get_signal_handler());
            assert_eq!(obs_get_proc_handler(), oracle_obs_get_proc_handler());
            assert_eq!(obs_get_locale() as usize, fields[0]);
            assert_eq!(obs_get_signal_handler() as usize, fields[1]);
            assert_eq!(obs_get_proc_handler() as usize, fields[2]);
        }
    }
}

#[test]
fn global_reads_match_c() {
    for fields in [
        [0, 0, 0],
        [1, 0, 0],
        [0, 2, 0],
        [0, 0, 3],
        [1, 2, 3],
        [3, 1, 2],
        [7, 7, 7],
        [usize::MAX, 1, 0],
        [0, 0, 0],
    ] {
        compare(fields);
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &proptest::collection::vec(any::<[usize; 3]>(), 0..32),
            |sequence| {
                for fields in sequence {
                    compare(fields);
                }
                Ok(())
            },
        )
        .unwrap();
}
