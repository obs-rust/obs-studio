//! Tier 3: the original C counters and the Rust exports, each calling the
//! tracing `obs_source_activate()` / `obs_source_deactivate()` doubles of
//! oracle/view.c, with exact diagnostics: the same call with the same source
//! and view type for a source, and the same single `LOG_DEBUG` line for
//! NULL. No intentional differences or excluded inputs.
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_c_oracle::view::OracleViewSource;
use obs_source::ffi::source_show_active_counters::{
    obs_source_dec_active, obs_source_dec_showing, obs_source_inc_active, obs_source_inc_showing,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Call = unsafe extern "C" fn(*mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_show_active_counters_check(
        call: Call,
        which: c_int,
        get: GetLog,
        set: SetLog,
        source: *mut c_void,
        observed: *mut c_char,
        observed_size: usize,
    ) -> bool;
}

/// The four exports, in the oracle's `which` order.
const REAL: [Call; 4] = [
    obs_source_inc_showing,
    obs_source_inc_active,
    obs_source_dec_showing,
    obs_source_dec_active,
];

/// What both sides shared (the trace for a source, the log line for NULL),
/// after checking they agree on logs and calls.
fn check_with(call: Call, which: usize, source: Option<&CStr>) -> Option<String> {
    let mut test_source = source.map(|name| OracleViewSource {
        name: name.as_ptr(),
        removed: false,
        dying: false,
    });
    let pointer = test_source
        .as_mut()
        .map_or(core::ptr::null_mut(), |s| core::ptr::from_mut(s).cast());
    let mut observed = [0 as c_char; 256];
    // SAFETY: the isolated child owns the log handler and the view trace
    // hook; the source is an oracle/view.c test source that outlives the
    // call, or NULL; the buffer is writable for its length.
    let equal = unsafe {
        oracle_source_show_active_counters_check(
            call,
            c_int::try_from(which).unwrap(),
            base_get_log_handler,
            base_set_log_handler,
            pointer,
            observed.as_mut_ptr(),
            observed.len(),
        )
    };
    // SAFETY: the oracle wrote a NUL-terminated string into the buffer.
    let observed = unsafe { CStr::from_ptr(observed.as_ptr()) };
    equal.then(|| observed.to_str().unwrap().to_owned())
}

fn check(which: usize, source: Option<&CStr>) -> String {
    check_with(REAL[which], which, source)
        .unwrap_or_else(|| panic!("which={which}, source={source:?}"))
}

/// Mutants of the shims: each must disagree with the original somewhere.
unsafe extern "C" fn wrong_view(source: *mut c_void) {
    // SAFETY: as the real export: inc_active's call, inc_showing's name.
    unsafe { obs_source_inc_active(source) }
}
unsafe extern "C" fn wrong_direction(source: *mut c_void) {
    // SAFETY: as the real export.
    unsafe { obs_source_dec_showing(source) }
}
unsafe extern "C" fn silent_null(source: *mut c_void) {
    if !source.is_null() {
        // SAFETY: a live oracle source.
        unsafe { obs_source_inc_showing(source) }
    }
}
unsafe extern "C" fn forwards_null(_source: *mut c_void) {
    let mut stand_in = OracleViewSource {
        name: c"null".as_ptr(),
        removed: false,
        dying: false,
    };
    // SAFETY: a live oracle source.
    unsafe { obs_source_inc_showing(core::ptr::from_mut(&mut stand_in).cast()) }
}

#[test]
fn show_active_counters_calls_and_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::show_active_counters_calls_and_diagnostics_match_c"
    )) {
        return;
    }
    // The expected values pin the shared result, so a mutation on both sides
    // (or in the oracle) still fails: MAIN_VIEW is 1 and AUX_VIEW 2.
    let expected = [
        ("obs_source_inc_showing", "activate a 2"),
        ("obs_source_inc_active", "activate a 1"),
        ("obs_source_dec_showing", "deactivate a 2"),
        ("obs_source_dec_active", "deactivate a 1"),
    ];
    for (which, (name, trace)) in expected.into_iter().enumerate() {
        assert_eq!(check(which, Some(c"a")), trace);
        assert_eq!(
            check(which, None),
            format!("{name}: Null 'source' parameter")
        );
    }
    let mutants: [Call; 4] = [wrong_view, wrong_direction, silent_null, forwards_null];
    for mutant in mutants {
        let caught = [Some(c"a"), None]
            .into_iter()
            .any(|source| check_with(mutant, 0, source).is_none());
        assert!(caught, "a mutant of the shims agreed with the original C");
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(0usize..4, proptest::option::of("[a-z]{1,12}")),
            |(which, name)| {
                let name = name.map(|n| std::ffi::CString::new(n).unwrap());
                let observed = check(which, name.as_deref());
                match &name {
                    Some(name) => prop_assert_eq!(
                        observed,
                        format!(
                            "{} {} {}",
                            expected[which].1.split(' ').next().unwrap(),
                            name.to_str().unwrap(),
                            expected[which].1.rsplit(' ').next().unwrap()
                        )
                    ),
                    None => prop_assert_eq!(
                        observed,
                        format!("{}: Null 'source' parameter", expected[which].0)
                    ),
                }
                Ok(())
            },
        )
        .unwrap();
}
