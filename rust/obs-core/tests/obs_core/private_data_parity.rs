//! Tier 3: original C wrappers and Rust exports over real-header core storage.
//! No intentional differences. The core is always initialized; concurrent core
//! destruction and invalid data pointers are outside the original C contract.
use obs_core::ffi::private_data::{obs_get_private_data, obs_set_private_data};
use obs_data::ffi::obs_data::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{CStr, c_void};

unsafe extern "C" {
    fn oracle_core_private_data_set(data: *mut c_void);
    fn oracle_obs_set_private_data(settings: *mut c_void);
    fn oracle_obs_get_private_data() -> *mut c_void;
}

struct State {
    data: *mut c_void,
    settings: *mut c_void,
    empty: *mut c_void,
    held: Vec<*mut c_void>,
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
                held: Vec::new(),
                rust,
            }
        }
    }

    fn set(&mut self, settings: *mut c_void) {
        // SAFETY: the TLS core's private-data field and optional settings are
        // live objects owned here. This fixture never models a missing core.
        unsafe {
            oracle_core_private_data_set(self.data);
            if self.rust {
                obs_set_private_data(settings);
            } else {
                oracle_obs_set_private_data(settings);
            }
        }
    }

    fn get(&mut self) {
        // SAFETY: the TLS core and its data are live; retain is stored until Drop.
        unsafe {
            oracle_core_private_data_set(self.data);
            let before = ref_count_for_tests(self.data);
            let result = if self.rust {
                obs_get_private_data()
            } else {
                oracle_obs_get_private_data()
            };
            assert_eq!(result, self.data);
            assert_ne!(result, self.settings);
            assert_eq!(ref_count_for_tests(self.data), before + 1);
            self.held.push(result);
        }
    }

    fn step(&mut self, op: u8, value: i64) {
        // SAFETY: all three data objects are live, independently owned references.
        unsafe {
            match op % 9 {
                0 => obs_data_set_int(self.settings, c"value".as_ptr(), value),
                1 => obs_data_set_string(self.data, c"old".as_ptr(), c"discard".as_ptr()),
                2 => obs_data_set_default_int(self.data, c"value".as_ptr(), value),
                3 => self.set(std::ptr::null_mut()),
                4 => self.set(self.empty),
                5 => self.set(self.settings),
                6 => self.set(self.data),
                7 => self.get(),
                _ => obs_data_set_bool(self.settings, c"flag".as_ptr(), value != 0),
            }
        }
    }

    fn snapshot(&self) -> (String, String, usize, [bool; 3]) {
        // SAFETY: data remains live; each borrowed JSON buffer is copied before
        // another call can replace it. No reference is added by these reads.
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
                [
                    obs_data_has_user_value(self.data, c"value".as_ptr()),
                    obs_data_has_default_value(self.data, c"value".as_ptr()),
                    obs_data_has_user_value(self.data, c"old".as_ptr()),
                ],
            )
        }
    }

    fn shutdown(&mut self) {
        self.get();
        // SAFETY: drop the core's reference; retained getter results keep the
        // object alive, exactly as obs_shutdown does in the unchanged C test.
        unsafe {
            let value = obs_data_get_int(self.data, c"value".as_ptr());
            obs_data_release(self.data);
            self.data = std::ptr::null_mut();
            for &held in &self.held {
                assert_eq!(obs_data_get_int(held, c"value".as_ptr()), value);
            }
            assert_eq!(ref_count_for_tests(self.held[0]), self.held.len());
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        // SAFETY: release every owned initial/returned reference exactly once.
        unsafe {
            oracle_core_private_data_set(std::ptr::null_mut());
            obs_data_release(self.data);
            obs_data_release(self.settings);
            obs_data_release(self.empty);
            for data in self.held.drain(..) {
                obs_data_release(data);
            }
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
    rust.shutdown();
    c.shutdown();
}

#[test]
fn original_c_matches_replacement_and_retained_getter() {
    compare(&[]);
    // Each branch, replacement after old values, defaults, NULL, empty,
    // self-assignment, independent settings mutation and repeated retained reads.
    compare(&[
        (1, 0),
        (0, 42),
        (8, 1),
        (5, 0),
        (0, 99),
        (7, 0),
        (7, 0),
        (2, 17),
        (3, 0),
        (1, 0),
        (4, 0),
        (5, 0),
        (6, 0),
        (7, 0),
    ]);
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        compare(&[(0, value), (5, 0), (2, value), (6, 0), (7, 0)]);
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
