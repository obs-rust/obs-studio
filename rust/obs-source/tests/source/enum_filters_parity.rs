//! Tier 3: the original C `obs_source_enum_filters()` and the Rust shim on
//! noise-filled real-header sources, with exact diagnostics.
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_source::ffi::source_enum_filters::{EnumProc, obs_source_enum_filters};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type EnumQuery = unsafe extern "C" fn(*mut c_void, Option<EnumProc>, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_enum_filters_check(
        query: EnumQuery,
        get: GetLog,
        set: SetLog,
        present: bool,
        callback: bool,
        noise: u8,
        count: usize,
        mode: c_int,
        calls: *mut usize,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn obs_source_enum_filters_lock(source: *mut c_void);
    fn obs_source_enum_filters_count(source: *const c_void) -> usize;
    fn obs_source_enum_filters_at(source: *const c_void, index: usize) -> *mut c_void;
    fn obs_source_enum_filters_unlock(source: *mut c_void);
}

const FILTERS: usize = 8;
const MODES: c_int = 4;

fn message(buffer: &[c_char]) -> String {
    // SAFETY: the oracle NUL-terminated the buffer.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// The original's call count and log line, after checking both sides agree.
fn enum_with(
    query: EnumQuery,
    present: bool,
    callback: bool,
    noise: u8,
    count: usize,
    mode: c_int,
) -> Option<(usize, String)> {
    let mut calls = 0;
    let mut buffer = [0 as c_char; 256];
    // SAFETY: the isolated child owns the log handler. C builds live
    // fixtures, runs both enumerations, compares and restores logging; the
    // buffer is as long as passed.
    let equal = unsafe {
        oracle_enum_filters_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            callback,
            noise,
            count,
            mode,
            &raw mut calls,
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    equal.then(|| (calls, message(&buffer)))
}

fn enumerate(
    present: bool,
    callback: bool,
    noise: u8,
    count: usize,
    mode: c_int,
) -> (usize, String) {
    enum_with(
        obs_source_enum_filters,
        present,
        callback,
        noise,
        count,
        mode,
    )
    .unwrap_or_else(|| {
        panic!(
            "present={present}, callback={callback}, noise={noise:#x}, count={count}, mode={mode}"
        )
    })
}

/// Mutants: each must disagree with the original somewhere.
unsafe extern "C" fn enum_forward(
    source: *mut c_void,
    callback: Option<EnumProc>,
    param: *mut c_void,
) {
    let (false, Some(callback)) = (source.is_null(), callback) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_enum_filters(source, callback, param) };
    };
    // SAFETY: a live fixture source, its list read under its lock.
    unsafe {
        obs_source_enum_filters_lock(source);
        for index in 0..obs_source_enum_filters_count(source) {
            callback(source, obs_source_enum_filters_at(source, index), param);
        }
        obs_source_enum_filters_unlock(source);
    }
}
unsafe extern "C" fn enum_silent_null_callback(
    source: *mut c_void,
    callback: Option<EnumProc>,
    param: *mut c_void,
) {
    if callback.is_none() {
        return;
    }
    // SAFETY: as the export's.
    unsafe { obs_source_enum_filters(source, callback, param) }
}
unsafe extern "C" fn enum_snapshot(
    source: *mut c_void,
    callback: Option<EnumProc>,
    param: *mut c_void,
) {
    let (false, Some(callback)) = (source.is_null(), callback) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_enum_filters(source, callback, param) };
    };
    // SAFETY: a live fixture source, its list copied under its lock.
    unsafe {
        obs_source_enum_filters_lock(source);
        let count = obs_source_enum_filters_count(source);
        let entries: Vec<_> = (0..count)
            .map(|index| obs_source_enum_filters_at(source, index))
            .collect();
        for filter in entries.into_iter().rev() {
            callback(source, filter, param);
        }
        obs_source_enum_filters_unlock(source);
    }
}
unsafe extern "C" fn enum_unlocked(
    source: *mut c_void,
    callback: Option<EnumProc>,
    param: *mut c_void,
) {
    let (false, Some(callback)) = (source.is_null(), callback) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_enum_filters(source, callback, param) };
    };
    // SAFETY: a live fixture source, read without its lock.
    unsafe {
        for index in (0..obs_source_enum_filters_count(source)).rev() {
            callback(source, obs_source_enum_filters_at(source, index), param);
        }
    }
}

const SOURCE_LINE: &str = "obs_source_enum_filters: Null 'source' parameter";
const CALLBACK_LINE: &str = "obs_source_enum_filters: Null 'callback_obj.obj' parameter";

/// Each shim mutant disagrees with the original C somewhere.
fn rejects_mutants() {
    let mutants: [EnumQuery; 4] = [
        enum_forward,
        enum_silent_null_callback,
        enum_snapshot,
        enum_unlocked,
    ];
    for mutant in mutants {
        let caught = (0..MODES).any(|mode| {
            [(true, false, 3), (true, true, 4)]
                .into_iter()
                .any(|(present, callback, count)| {
                    enum_with(mutant, present, callback, 0x5a, count, mode).is_none()
                })
        });
        assert!(caught, "an enumeration mutant agreed with the original C");
    }
}

#[test]
fn enum_filters_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::enum_filters_matches_c")) {
        return;
    }
    for noise in [0, 0x5a, 0xff] {
        for mode in 0..MODES {
            assert_eq!(
                enumerate(false, true, noise, 3, mode),
                (0, SOURCE_LINE.to_owned())
            );
            assert_eq!(
                enumerate(false, false, noise, 3, mode),
                (0, SOURCE_LINE.to_owned())
            );
            assert_eq!(
                enumerate(true, false, noise, 3, mode),
                (0, CALLBACK_LINE.to_owned())
            );
            for count in 0..=FILTERS {
                assert_eq!(
                    enumerate(true, true, noise, count, mode),
                    (count, String::new())
                );
            }
        }
    }

    rejects_mutants();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<(bool, bool, u8)>(), 0..=FILTERS, 0..MODES),
            |((present, callback, noise), count, mode)| {
                let (calls, _) = enumerate(present, callback, noise, count, mode);
                prop_assert_eq!(calls, if present && callback { count } else { 0 });
                Ok(())
            },
        )
        .unwrap();
}
