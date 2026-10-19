//! `obs_data_t` shared across threads (#1451).
//!
//! C settings objects are read and written from several threads (the
//! video and graphics threads read source settings while the UI writes
//! them). libobs makes only the reference count atomic; concurrent reads
//! are safe there. These tests read, count and write one object from
//! several threads at once, through the C ABI.

use std::ffi::c_void;
use std::thread;

use obs_data::ffi::obs_data::{
    obs_data_addref, obs_data_create, obs_data_get_int, obs_data_release, obs_data_set_int,
    ref_count_for_tests,
};

const THREADS: usize = 8;
const ROUNDS: usize = 50_000;

/// A raw `obs_data_t` that the test moves to other threads, as C does.
#[derive(Clone, Copy)]
struct Shared(*mut c_void);
// SAFETY: the C API allows the object on any thread; these tests check that
// the Rust side does.
unsafe impl Send for Shared {}

#[test]
fn concurrent_addref_and_release_keep_the_count() {
    // SAFETY: a fresh object, used through the C ABI only.
    let data = Shared(unsafe { obs_data_create() });
    thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(move || {
                let d = data;
                for _ in 0..ROUNDS {
                    // SAFETY: the creator's reference keeps it alive.
                    unsafe {
                        obs_data_addref(d.0);
                        obs_data_release(d.0);
                    }
                }
            });
        }
    });
    // SAFETY: still live; one reference, the creator's.
    unsafe {
        assert_eq!(ref_count_for_tests(data.0), 1);
        obs_data_release(data.0);
    }
}

#[test]
fn concurrent_reads_and_writes_see_whole_values() {
    // SAFETY: a fresh object, used through the C ABI only.
    let data = Shared(unsafe { obs_data_create() });
    // SAFETY: as above.
    unsafe { obs_data_set_int(data.0, c"n".as_ptr(), 0) };
    thread::scope(|scope| {
        for t in 0..THREADS {
            scope.spawn(move || {
                let d = data;
                for i in 0..ROUNDS {
                    // SAFETY: the creator's reference keeps it alive.
                    unsafe {
                        if t == 0 {
                            obs_data_set_int(d.0, c"n".as_ptr(), i as i64);
                        } else {
                            let n = obs_data_get_int(d.0, c"n".as_ptr());
                            assert!((0..ROUNDS as i64).contains(&n), "torn value {n}");
                        }
                    }
                }
            });
        }
    });
    // SAFETY: still live; one reference, the creator's.
    unsafe {
        assert_eq!(ref_count_for_tests(data.0), 1);
        obs_data_release(data.0);
    }
}
