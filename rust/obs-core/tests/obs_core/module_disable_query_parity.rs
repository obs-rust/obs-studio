//! Original-C parity for `obs_get_module_allow_disable` (#5183): the thin
//! ABI shim against the original wrapper and its retained membership helper.
//! Both call the oracle's recording `obs_in_core_module_list()` and the
//! existing Rust policy. Each must ask membership exactly once with the
//! caller's own pointer, leave the name untouched and return the same
//! answer. NULL names are outside the matcher's domain and excluded. No
//! intentional differences.
use core::ffi::{CStr, c_char};
use std::ffi::CString;

use obs_core::ffi::module_disable_query::obs_get_module_allow_disable;
use obs_core::module_disable_query::{Dependencies, get_module_allow_disable};
use obs_core::module_load_rules::allow_disable;
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const c_char) -> bool;

unsafe extern "C" {
    fn oracle_module_disable_query_check(
        mode: i32,
        name: *const c_char,
        port: Query,
        out: *mut bool,
    ) -> bool;
}

/// The membership answer the oracle reported, then the real Rust policy:
/// the safe core must give the original C's answer from the same inputs.
struct Answered(bool);

impl Dependencies<()> for Answered {
    fn in_core_module_list(&mut self, (): ()) -> bool {
        self.0
    }

    fn allow_disable(&mut self, core_name: bool) -> bool {
        allow_disable(core_name)
    }
}

fn check(mode: i32, name: &CStr) {
    let mut out = [false; 2];
    // SAFETY: `name` is a live NUL-terminated string for the call; C writes
    // two results to `out` and calls the port only with `name`.
    assert!(
        unsafe {
            oracle_module_disable_query_check(
                mode,
                name.as_ptr(),
                obs_get_module_allow_disable,
                out.as_mut_ptr(),
            )
        },
        "mode {mode} name {name:?}"
    );
    let [original, member] = out;
    match mode {
        1 => assert!(!member),
        2 => assert!(member),
        _ => {}
    }
    assert_eq!(
        get_module_allow_disable(&mut Answered(member), ()),
        original,
        "mode {mode} name {name:?}"
    );
}

const LISTED: [&CStr; 6] = [
    c"obs-ffmpeg",
    c"image-source",
    c"obs-x264",
    c"Mixed-Case",
    c"mIXED-cASE",
    c"rtmp-services",
];

const UNLISTED: [&CStr; 10] = [
    c"",
    c"obs",
    c"OBS-FFMPEG",
    c"Obs-ffmpeg",
    c"obs-ffmpeg ",
    c" obs-ffmpeg",
    c"obs-ffmpe",
    c"image-source.so",
    c"mixed-case",
    c"obs-\xC3\xA9",
];

#[test]
fn characterized_names_match_original_c() {
    for mode in 0..3 {
        for name in LISTED.iter().chain(&UNLISTED) {
            check(mode, name);
        }
    }
}

#[test]
fn listed_names_are_core_and_case_sensitive() {
    for name in LISTED {
        let mut out = [false; 2];
        // SAFETY: as in `check`.
        assert!(unsafe {
            oracle_module_disable_query_check(
                0,
                name.as_ptr(),
                obs_get_module_allow_disable,
                out.as_mut_ptr(),
            )
        });
        assert_eq!(out, [false, true], "{name:?}");
    }
    for name in UNLISTED {
        let mut out = [true; 2];
        // SAFETY: as in `check`.
        assert!(unsafe {
            oracle_module_disable_query_check(
                0,
                name.as_ptr(),
                obs_get_module_allow_disable,
                out.as_mut_ptr(),
            )
        });
        assert_eq!(out, [true, false], "{name:?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_names_match_original_c(
        mode in 0..3i32,
        listed in proptest::option::of(0..LISTED.len()),
        name in "[a-zA-Z0-9_. -]{0,16}",
    ) {
        let name = match listed {
            Some(index) => LISTED[index].to_owned(),
            None => CString::new(name).expect("generated names have no NUL"),
        };
        check(mode, &name);
    }
}
