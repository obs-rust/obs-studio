//! Tier 3: compare C and the ABI boundary with the same recording collaborators.
//! Invalid cache links, null effect_path fields and unterminated strings are C
//! UB and excluded. No intentional behavior differences.
#![allow(unsafe_code)]
use core::ffi::{CStr, c_char};
use core::ptr;
use obs_c_oracle as _;
use obs_graphics::ffi::effect_file::gs_effect_create_from_file;
use obs_graphics::ffi::effect_lookups::gs_effect;
use proptest::prelude::*;
use std::ffi::CString;

unsafe extern "C" {
    fn oracle_effect_file_reset(context: bool, read_ok: bool, create_ok: bool);
    fn oracle_effect_file_entry(index: usize, path: *mut c_char);
    fn oracle_effect_file_run(file: *const c_char, error: *mut *mut c_char) -> *mut gs_effect;
    fn oracle_effect_file_args(file: *const c_char, error: *mut *mut c_char);
    fn oracle_effect_file_events() -> *const c_char;
    fn oracle_effect_file_message() -> *const c_char;
    fn oracle_effect_file_level() -> i32;
    fn oracle_effect_file_args_ok() -> bool;
    fn oracle_effect_file_forwarded() -> bool;
}
#[derive(Debug, PartialEq)]
struct Snapshot {
    result: *mut gs_effect,
    error: *mut c_char,
    events: Vec<u8>,
    message: Vec<u8>,
    level: i32,
    args_ok: bool,
    forwarded: bool,
}
#[derive(Clone, Debug)]
struct Case {
    context: bool,
    read_ok: bool,
    create_ok: bool,
    with_error: bool,
    file: Option<CString>,
    paths: Vec<CString>,
}
fn run(case: &Case, rust: bool) -> Snapshot {
    let mut error = c"unchanged".as_ptr().cast_mut();
    let error_arg = if case.with_error {
        &mut error
    } else {
        ptr::null_mut()
    };
    let file = case.file.as_ref().map_or(ptr::null(), |s| s.as_ptr());
    // SAFETY: this thread owns the fixture's TLS. All paths are live C strings,
    // at most seven acyclic cache entries; callbacks operate on this fixture.
    unsafe {
        oracle_effect_file_reset(case.context, case.read_ok, case.create_ok);
        for (i, path) in case.paths.iter().enumerate() {
            oracle_effect_file_entry(i, path.as_ptr().cast_mut());
        }
        oracle_effect_file_args(file, error_arg);
        let result = if rust {
            gs_effect_create_from_file(file, error_arg)
        } else {
            oracle_effect_file_run(file, error_arg)
        };
        Snapshot {
            result,
            error,
            events: CStr::from_ptr(oracle_effect_file_events())
                .to_bytes()
                .to_vec(),
            message: CStr::from_ptr(oracle_effect_file_message())
                .to_bytes()
                .to_vec(),
            level: oracle_effect_file_level(),
            args_ok: oracle_effect_file_args_ok(),
            forwarded: oracle_effect_file_forwarded(),
        }
    }
}
fn check(case: &Case) {
    assert_eq!(run(case, true), run(case, false), "{case:?}");
}
#[test]
fn validation_cache_and_failure_edges() {
    for context in [false, true] {
        for read_ok in [false, true] {
            for create_ok in [false, true] {
                for with_error in [false, true] {
                    for file in [
                        None,
                        Some(c"".to_owned()),
                        Some(c"case.effect".to_owned()),
                        Some(CString::new(vec![0xff, 0xfe]).unwrap()),
                    ] {
                        for paths in [
                            vec![],
                            vec![
                                c"CASE.effect".to_owned(),
                                c"case.effect".to_owned(),
                                c"case.effect".to_owned(),
                            ],
                            vec![c"unmatched".to_owned()],
                        ] {
                            check(&Case {
                                context,
                                read_ok,
                                create_ok,
                                with_error,
                                file: file.clone(),
                                paths,
                            });
                        }
                    }
                }
            }
        }
    }
}
fn path() -> impl Strategy<Value = CString> {
    prop::collection::vec(1_u8..=255, 0..24).prop_map(|bytes| CString::new(bytes).unwrap())
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(96)))]
    #[test]
    fn original_c_matches_rust(
        context in any::<bool>(), read_ok in any::<bool>(), create_ok in any::<bool>(),
        with_error in any::<bool>(), file in prop::option::of(path()),
        paths in prop::collection::vec(path(), 0..7), hit in 0_usize..8,
    ) {
        let file = paths.get(hit).cloned().or(file);
        check(&Case { context, read_ok, create_ok, with_error, file, paths });
    }
}
