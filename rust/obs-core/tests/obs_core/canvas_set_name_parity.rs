//! Tier 3: unchanged extracted C setter vs public Rust ABI on real-header
//! canvases and production adapters. Context naming and delivery are shared
//! stand-ins; the unchanged actual-library Tier 2 suites cover those dependencies.
//! No exclusions or intentional differences on defined C inputs.
use core::ffi::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::sync::Mutex;

use obs_core::ffi::canvas_set_name::obs_canvas_set_name;
use proptest::prelude::*;

static LOCK: Mutex<()> = Mutex::new(());
unsafe extern "C" {
    fn oracle_obs_canvas_set_name(canvas: *mut c_void, name: *const c_char);
    fn oracle_canvas_rename_new(
        name: *const c_char,
        taken: *const c_char,
        flags: u32,
        private: bool,
        flip: bool,
    ) -> *mut c_void;
    fn oracle_canvas_rename_select(canvas: *mut c_void);
    fn oracle_canvas_rename_string(canvas: *mut c_void, which: u32) -> *const c_char;
    fn oracle_canvas_rename_count(canvas: *mut c_void, which: u32) -> u32;
    fn oracle_canvas_rename_free(canvas: *mut c_void);
}

#[derive(Debug, PartialEq)]
struct Outcome {
    strings: Vec<Option<Vec<u8>>>,
    counts: Vec<u32>,
}

fn run(
    rust: bool,
    old: &CStr,
    taken: &CStr,
    flags: u32,
    private: bool,
    flip: bool,
    names: &[Option<CString>],
) -> Vec<Outcome> {
    // SAFETY: the fixture owns real C layouts, copies names and lives until freed.
    unsafe {
        let canvas = oracle_canvas_rename_new(old.as_ptr(), taken.as_ptr(), flags, private, flip);
        assert!(!canvas.is_null());
        oracle_canvas_rename_select(canvas);
        let mut outcomes = Vec::new();
        for name in names {
            let name = name
                .as_ref()
                .map_or(core::ptr::null(), |name| name.as_ptr());
            if rust {
                obs_canvas_set_name(canvas, name);
            } else {
                oracle_obs_canvas_set_name(canvas, name);
            }
            let strings = (0..5)
                .map(|which| {
                    let value = oracle_canvas_rename_string(canvas, which);
                    if value.is_null() {
                        None
                    } else {
                        Some(CStr::from_ptr(value).to_bytes().to_vec())
                    }
                })
                .collect();
            let counts = (0..4)
                .map(|which| oracle_canvas_rename_count(canvas, which))
                .collect();
            outcomes.push(Outcome { strings, counts });
        }
        oracle_canvas_rename_free(canvas);
        outcomes
    }
}
fn compare(
    old: &[u8],
    taken: &[u8],
    flags: u32,
    private: bool,
    flip: bool,
    names: Vec<Option<Vec<u8>>>,
) {
    let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let old = CString::new(old).unwrap();
    let taken = CString::new(taken).unwrap();
    let names = names
        .into_iter()
        .map(|name| name.map(|name| CString::new(name).unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        run(true, &old, &taken, flags, private, flip, &names),
        run(false, &old, &taken, flags, private, flip, &names)
    );
}

#[test]
fn exact_setter_edges_and_callback_mutations() {
    use obs_core::canvas_rules::{MAIN, REMOVED};
    for flags in [0, MAIN, REMOVED, MAIN | REMOVED, u32::MAX] {
        for private in [false, true] {
            for flip in [false, true] {
                compare(
                    b"K 2",
                    b"K",
                    flags,
                    private,
                    flip,
                    vec![
                        None,
                        Some(vec![]),
                        Some(b"K 2".to_vec()),
                        Some(b"K".to_vec()),
                        Some(b"Main".to_vec()),
                        Some(b"\xff%name".to_vec()),
                    ],
                );
            }
        }
    }
    // The empty request must not inspect even a null canvas.
    // SAFETY: both implementations return before touching canvas for these names.
    unsafe {
        obs_canvas_set_name(core::ptr::null_mut(), core::ptr::null());
        oracle_obs_canvas_set_name(core::ptr::null_mut(), core::ptr::null());
        obs_canvas_set_name(core::ptr::null_mut(), c"".as_ptr());
        oracle_obs_canvas_set_name(core::ptr::null_mut(), c"".as_ptr());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn exact_setter_random_sequences(
        old in prop::collection::vec(1u8..=255, 0..40),
        taken in prop::collection::vec(1u8..=255, 0..40),
        flags in any::<u32>(), private in any::<bool>(), flip in any::<bool>(),
        names in prop::collection::vec(prop::option::of(prop::collection::vec(1u8..=255, 0..40)), 0..20)
    ) {
        compare(&old, &taken, flags, private, flip, names);
    }
}
