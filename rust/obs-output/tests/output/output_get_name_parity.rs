//! Tier 3: the untouched C body against the Rust shim on a real-layout
//! output, through the production context-name adapter. No intentional
//! differences.
use core::ffi::{c_char, c_int, c_void};
use obs_output::ffi::output_get_name::obs_output_get_name;
use std::ffi::CString;

type Getter = unsafe extern "C" fn(*const c_void) -> *const c_char;
unsafe extern "C" {
    fn oracle_output_get_name_probe(
        get: Getter,
        name: *const c_char,
        renamed: *const c_char,
        null_name: bool,
    ) -> bool;
    fn obs_output_get_name_context_name(output: *const c_void) -> *const c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

fn probe(get: Getter, name: &[u8], renamed: &[u8], null_name: bool) -> bool {
    let name = CString::new(name).unwrap();
    let renamed = CString::new(renamed).unwrap();
    // SAFETY: C copies the bounded strings and passes NULL or a live
    // real-layout output to both getters. The isolated process owns the log
    // handler, which the probe restores.
    unsafe { oracle_output_get_name_probe(get, name.as_ptr(), renamed.as_ptr(), null_name) }
}

#[test]
fn original_c_characterization_and_edges() {
    if !super::isolated(concat!(
        module_path!(),
        "::original_c_characterization_and_edges"
    )) {
        return;
    }
    for name in [
        b"get name".as_slice(),
        b"__unnamed0003",
        b"before",
        b"",
        b"%s-%n",
        b"\xff\x80",
    ] {
        for null_name in [false, true] {
            assert!(probe(obs_output_get_name, name, b"after", null_name));
        }
    }
}

/// Reads the field but never logs for NULL.
unsafe extern "C" fn silent_null(output: *const c_void) -> *const c_char {
    if output.is_null() {
        return core::ptr::null();
    }
    // SAFETY: a live probe output.
    unsafe { obs_output_get_name_context_name(output) }
}

/// Logs the NULL diagnostic with another function name.
unsafe extern "C" fn wrong_function(output: *const c_void) -> *const c_char {
    if output.is_null() {
        // SAFETY: static format and arguments.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_output_get_last_error".as_ptr(),
                c"output".as_ptr(),
            )
        };
        return core::ptr::null();
    }
    // SAFETY: a live probe output.
    unsafe { obs_output_get_name_context_name(output) }
}

/// Returns NULL for a live output.
unsafe extern "C" fn always_null(output: *const c_void) -> *const c_char {
    // SAFETY: the real shim, called for its NULL diagnostic only.
    let _ = unsafe { obs_output_get_name(output) };
    core::ptr::null()
}

std::thread_local! {
    static CACHED: core::cell::Cell<*const c_char> = const { core::cell::Cell::new(core::ptr::null()) };
}

/// Caches the first name it reads, so a later rename is missed.
unsafe extern "C" fn cached(output: *const c_void) -> *const c_char {
    // SAFETY: the real shim on the probe's NULL or live output.
    let name = unsafe { obs_output_get_name(output) };
    if output.is_null() {
        return name;
    }
    CACHED.with(|c| {
        if c.get().is_null() {
            c.set(name);
        }
        c.get()
    })
}

#[test]
fn mutants_are_caught() {
    if !super::isolated(concat!(module_path!(), "::mutants_are_caught")) {
        return;
    }
    for mutant in [silent_null as Getter, wrong_function, always_null, cached] {
        CACHED.with(|c| c.set(core::ptr::null()));
        assert!(!probe(mutant, b"before", b"after", false));
    }
}

#[test]
fn randomized_original_c_parity() {
    if !super::isolated(concat!(module_path!(), "::randomized_original_c_parity")) {
        return;
    }
    use proptest::prelude::*;
    use proptest::test_runner::{Config, TestRunner};
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(
                proptest::collection::vec(1_u8..=255, 0..128),
                proptest::collection::vec(1_u8..=255, 0..128),
                any::<bool>(),
            ),
            |(name, renamed, null_name)| {
                prop_assert!(probe(obs_output_get_name, &name, &renamed, null_name));
                Ok(())
            },
        )
        .unwrap();
}
