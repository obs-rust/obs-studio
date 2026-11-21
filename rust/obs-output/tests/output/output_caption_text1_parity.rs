//! Tier 3: the untouched C wrapper against the Rust shim, each over a
//! recording `obs_output_output_caption_text2()`. Same forwarded output and
//! text pointers, the same duration bit for bit, and either no log (a live
//! output) or exactly `obs_output_valid()`'s diagnostic (NULL). No
//! intentional differences.
use core::ffi::{c_char, c_int, c_void};
use obs_output::ffi::output_caption_text1::obs_output_output_caption_text1;

type Wrapper = unsafe extern "C" fn(*mut c_void, *const c_char);
unsafe extern "C" {
    fn oracle_output_caption_text1_probe(
        candidate: Wrapper,
        output_live: bool,
        text_live: bool,
    ) -> bool;
    fn obs_output_output_caption_text2(output: *mut c_void, text: *const c_char, duration: f64);
    fn blog(level: c_int, format: *const c_char, ...);
}

fn probe(candidate: Wrapper, output_live: bool, text_live: bool) -> bool {
    // SAFETY: the probe passes NULL or its own static output and text, which
    // the recording stand-in never dereferences. The isolated process owns
    // the log handler, which the probe restores.
    unsafe { oracle_output_caption_text1_probe(candidate, output_live, text_live) }
}

fn every_input(candidate: Wrapper) -> bool {
    [(false, false), (false, true), (true, false), (true, true)]
        .into_iter()
        .all(|(output, text)| probe(candidate, output, text))
}

#[test]
fn original_c_characterization_and_edges() {
    if !super::isolated(concat!(
        module_path!(),
        "::original_c_characterization_and_edges"
    )) {
        return;
    }
    assert!(every_input(obs_output_output_caption_text1));
}

fn log_null(function: &core::ffi::CStr) {
    // SAFETY: static format and arguments.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            c"output".as_ptr(),
        )
    }
}

/// Forwards a NULL output instead of stopping.
unsafe extern "C" fn forwards_null(output: *mut c_void, text: *const c_char) {
    if output.is_null() {
        log_null(c"obs_output_output_caption_text1");
    }
    // SAFETY: the recording stand-in.
    unsafe { obs_output_output_caption_text2(output, text, 2.0) }
}

/// Stops silently on NULL.
unsafe extern "C" fn silent_null(output: *mut c_void, text: *const c_char) {
    if !output.is_null() {
        // SAFETY: the recording stand-in.
        unsafe { obs_output_output_caption_text2(output, text, 2.0) }
    }
}

/// Names another function in the NULL diagnostic.
unsafe extern "C" fn wrong_function(output: *mut c_void, text: *const c_char) {
    if output.is_null() {
        log_null(c"obs_output_output_caption_text2");
        return;
    }
    // SAFETY: the recording stand-in.
    unsafe { obs_output_output_caption_text2(output, text, 2.0) }
}

/// Forwards the wrong duration.
unsafe extern "C" fn wrong_duration(output: *mut c_void, text: *const c_char) {
    if output.is_null() {
        log_null(c"obs_output_output_caption_text1");
        return;
    }
    // SAFETY: the recording stand-in.
    unsafe { obs_output_output_caption_text2(output, text, 2.000_000_1) }
}

/// Drops the caller's text.
unsafe extern "C" fn drops_text(output: *mut c_void, _text: *const c_char) {
    if output.is_null() {
        log_null(c"obs_output_output_caption_text1");
        return;
    }
    // SAFETY: the recording stand-in.
    unsafe { obs_output_output_caption_text2(output, core::ptr::null(), 2.0) }
}

/// Forwards twice.
unsafe extern "C" fn twice(output: *mut c_void, text: *const c_char) {
    // SAFETY: the real shim, then the recording stand-in.
    unsafe {
        obs_output_output_caption_text1(output, text);
        if !output.is_null() {
            obs_output_output_caption_text2(output, text, 2.0);
        }
    }
}

#[test]
fn mutants_are_caught() {
    if !super::isolated(concat!(module_path!(), "::mutants_are_caught")) {
        return;
    }
    for mutant in [
        forwards_null as Wrapper,
        silent_null,
        wrong_function,
        wrong_duration,
        drops_text,
        twice,
    ] {
        assert!(!every_input(mutant));
    }
}
