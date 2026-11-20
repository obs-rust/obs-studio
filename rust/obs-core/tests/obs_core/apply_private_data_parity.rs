//! Tier 3: the original obs_apply_private_data and the Rust export over
//! real-header core storage. No intentional differences. The core is
//! initialized whenever settings are non-NULL, as the C contract requires.
use obs_core::ffi::private_data::obs_apply_private_data;
use obs_data::ffi::obs_data::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{CStr, c_void};

unsafe extern "C" {
    fn oracle_core_private_data_set(data: *mut c_void);
    fn oracle_obs_apply_private_data(settings: *mut c_void);
}

struct State {
    data: *mut c_void,
    settings: *mut c_void,
    empty: *mut c_void,
    rust: bool,
}

impl State {
    fn new(rust: bool) -> Self {
        // SAFETY: each created reference is owned here and released by Drop.
        unsafe {
            Self {
                data: obs_data_create(),
                settings: obs_data_create(),
                empty: obs_data_create(),
                rust,
            }
        }
    }

    fn apply(&mut self, settings: *mut c_void) {
        // SAFETY: the TLS core's private-data field and optional settings are
        // live objects owned here.
        unsafe {
            oracle_core_private_data_set(self.data);
            if self.rust {
                obs_apply_private_data(settings);
            } else {
                oracle_obs_apply_private_data(settings);
            }
        }
    }

    fn step(&mut self, op: u8, value: i64) {
        // SAFETY: all three data objects are live, independently owned references.
        unsafe {
            match op % 9 {
                0 => obs_data_set_int(self.settings, c"value".as_ptr(), value),
                1 => obs_data_set_string(self.data, c"old".as_ptr(), c"keep".as_ptr()),
                2 => obs_data_set_default_int(self.data, c"value".as_ptr(), value),
                3 => self.apply(std::ptr::null_mut()),
                4 => self.apply(self.empty),
                5 => self.apply(self.settings),
                6 => self.apply(self.data),
                7 => obs_data_set_int(self.data, c"value".as_ptr(), value),
                _ => obs_data_set_bool(self.settings, c"flag".as_ptr(), value != 0),
            }
        }
    }

    fn snapshot(&self) -> (String, String, usize, usize, [bool; 3]) {
        // SAFETY: both objects remain live; each borrowed JSON buffer is copied
        // before another call can replace it. No reference is added.
        unsafe {
            (
                CStr::from_ptr(obs_data_get_json_with_defaults(self.data))
                    .to_str()
                    .unwrap()
                    .to_owned(),
                CStr::from_ptr(obs_data_get_json_with_defaults(self.settings))
                    .to_str()
                    .unwrap()
                    .to_owned(),
                ref_count_for_tests(self.data),
                ref_count_for_tests(self.settings),
                [
                    obs_data_has_user_value(self.data, c"value".as_ptr()),
                    obs_data_has_default_value(self.data, c"value".as_ptr()),
                    obs_data_has_user_value(self.data, c"old".as_ptr()),
                ],
            )
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        // SAFETY: release every owned reference exactly once.
        unsafe {
            oracle_core_private_data_set(std::ptr::null_mut());
            obs_data_release(self.data);
            obs_data_release(self.settings);
            obs_data_release(self.empty);
        }
    }
}

fn compare(sequence: &[(u8, i64)]) {
    let mut rust = State::new(true);
    let mut c = State::new(false);
    for &(op, value) in sequence {
        rust.step(op, value);
        c.step(op, value);
        assert_eq!(rust.snapshot(), c.snapshot(), "after {op}, {value}");
    }
}

#[test]
fn null_settings_never_read_the_core() {
    // With no storage at all, NULL settings must return before the field is
    // used; a reactivated read would hand NULL to obs_data_apply.
    // SAFETY: NULL settings are the documented no-op; the TLS field is NULL.
    unsafe {
        oracle_core_private_data_set(std::ptr::null_mut());
        obs_apply_private_data(std::ptr::null_mut());
        oracle_obs_apply_private_data(std::ptr::null_mut());
    }
}

#[test]
fn original_c_matches_merge() {
    compare(&[]);
    // Merge over old values and defaults, NULL, empty, self-application and
    // independent mutation of both objects after applying.
    compare(&[
        (1, 0),
        (0, 42),
        (8, 1),
        (5, 0),
        (0, 99),
        (2, 17),
        (3, 0),
        (4, 0),
        (6, 0),
        (7, -3),
        (5, 0),
    ]);
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        compare(&[(0, value), (5, 0), (2, value), (6, 0), (3, 0)]);
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &proptest::collection::vec((0u8..9, any::<i64>()), 0..64),
            |sequence| {
                compare(&sequence);
                Ok(())
            },
        )
        .unwrap();
}
