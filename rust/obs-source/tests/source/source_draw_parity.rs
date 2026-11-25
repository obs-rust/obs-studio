//! Tier 3: the unchanged original C (`libobs/obs-source-draw.c.h`, oracle
//! `oracle_obs_source_draw*`) versus the production Rust bridges
//! (`set_color_matrix_with` / `draw_with`) over one recorder for every
//! graphics call (`oracle/source_draw.c`).
//!
//! Both sides must leave the same log: each call with its arguments
//! (pointers as fixture ids, floats as bits, the matrix and range bytes the
//! sets copy), the log lines through the real `blog()` and the framebuffer
//! sRGB flag after. Every combination of entry, current effect, NULL
//! params, linear sRGB, the flag before and the NULL pointers (or flip),
//! over positions at and around 0 and the `i32` ends, then proptest cases
//! over any position, size and float bits. No intentional differences, so
//! nothing is excluded. The recorder is C statics, so runs take a lock;
//! the tests start no threads of their own.
use core::ffi::{c_char, c_int, c_void};
use obs_source::ffi::source_draw::{
    Calls, LOG_DEBUG, LOG_WARNING, Matrix4, draw_with, set_color_matrix_with,
};
use obs_source::source_draw::GS_FLIP_V;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;
use std::sync::{Mutex, PoisonError};

const LOG: usize = 4096;
const STATE: usize = 8;
const VALUES: usize = 24;

type MatrixFn = unsafe extern "C" fn(*const Matrix4, *const [f32; 3], *const [f32; 3]);
type DrawFn = unsafe extern "C" fn(*mut c_void, c_int, c_int, u32, u32, bool);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_obs_source_draw_set_color_matrix(
        matrix: *const Matrix4,
        min: *const [f32; 3],
        max: *const [f32; 3],
    );
    fn oracle_obs_source_draw(
        texture: *mut c_void,
        x: c_int,
        y: c_int,
        cx: u32,
        cy: u32,
        flip: bool,
    );
    fn oracle_source_draw_run(
        set_color_matrix: MatrixFn,
        draw: DrawFn,
        state: *const c_int,
        values: *const f32,
        x: c_int,
        y: c_int,
        cx: u32,
        cy: u32,
        get_log: Option<GetLog>,
        set_log: Option<SetLog>,
        log: *mut c_char,
        log_len: usize,
    ) -> bool;
    fn oracle_source_draw_values(out: *mut c_int);

    fn blog(level: c_int, format: *const c_char, ...);
    fn oracle_sd_get_effect() -> *mut c_void;
    fn oracle_sd_param(effect: *const c_void, name: *const c_char) -> *mut c_void;
    fn oracle_sd_set_matrix4(param: *mut c_void, val: *const Matrix4);
    fn oracle_sd_set_val(param: *mut c_void, val: *const c_void, size: usize);
    fn oracle_sd_linear_srgb() -> bool;
    fn oracle_sd_srgb_enabled() -> bool;
    fn oracle_sd_enable_srgb(enable: bool);
    fn oracle_sd_set_texture_srgb(param: *mut c_void, texture: *mut c_void);
    fn oracle_sd_set_texture(param: *mut c_void, texture: *mut c_void);
    fn oracle_sd_matrix_push();
    fn oracle_sd_matrix_translate3f(x: f32, y: f32, z: f32);
    fn oracle_sd_draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32);
    fn oracle_sd_matrix_pop();
}

/// The recording stand-ins and the real `blog()`, as the bridge's [`Calls`].
enum Rec {}

// SAFETY: each method is the recording stand-in of the C function its
// trait doc names, which only logs and answers from the run's state, or
// the real blog().
unsafe impl Calls for Rec {
    fn get_effect() -> *mut c_void {
        // SAFETY: logs only.
        unsafe { oracle_sd_get_effect() }
    }
    unsafe fn blog(level: c_int, format: *const c_char, args: &[*const c_char]) {
        // SAFETY: the bridge's static format and strings.
        unsafe {
            match *args {
                [] => blog(level, format),
                [a] => blog(level, format, a),
                [a, b, ..] => blog(level, format, a, b),
            }
        }
    }
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: logs the id and the static name.
        unsafe { oracle_sd_param(effect, name) }
    }
    unsafe fn set_matrix4(param: *mut c_void, matrix: *const Matrix4) {
        // SAFETY: logs the id and the 64 live bytes.
        unsafe { oracle_sd_set_matrix4(param, matrix) }
    }
    unsafe fn set_val(param: *mut c_void, val: *const c_void, size: usize) {
        // SAFETY: logs the id and the size live bytes.
        unsafe { oracle_sd_set_val(param, val, size) }
    }
    fn get_linear_srgb() -> bool {
        // SAFETY: logs only.
        unsafe { oracle_sd_linear_srgb() }
    }
    fn framebuffer_srgb_enabled() -> bool {
        // SAFETY: logs only.
        unsafe { oracle_sd_srgb_enabled() }
    }
    fn enable_framebuffer_srgb(enable: bool) {
        // SAFETY: logs only.
        unsafe { oracle_sd_enable_srgb(enable) }
    }
    unsafe fn set_texture_srgb(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: logs ids only.
        unsafe { oracle_sd_set_texture_srgb(param, texture) }
    }
    unsafe fn set_texture(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: logs ids only.
        unsafe { oracle_sd_set_texture(param, texture) }
    }
    fn matrix_push() {
        // SAFETY: logs only.
        unsafe { oracle_sd_matrix_push() }
    }
    fn matrix_translate3f(x: f32, y: f32, z: f32) {
        // SAFETY: logs only.
        unsafe { oracle_sd_matrix_translate3f(x, y, z) }
    }
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32) {
        // SAFETY: logs ids only.
        unsafe { oracle_sd_draw_sprite(texture, flip, width, height) }
    }
    fn matrix_pop() {
        // SAFETY: logs only.
        unsafe { oracle_sd_matrix_pop() }
    }
}

unsafe extern "C" fn rust_set_color_matrix(
    matrix: *const Matrix4,
    min: *const [f32; 3],
    max: *const [f32; 3],
) {
    // SAFETY: the fixture's live values or NULL.
    unsafe { set_color_matrix_with::<Rec>(matrix, min, max) }
}

unsafe extern "C" fn rust_draw(
    texture: *mut c_void,
    x: c_int,
    y: c_int,
    cx: u32,
    cy: u32,
    flip: bool,
) {
    // SAFETY: the fixture's texture token or NULL; the recorder never
    // dereferences it.
    unsafe { draw_with::<Rec>(texture, x, y, cx, cy, flip) }
}

struct Case {
    state: [c_int; STATE],
    values: [f32; VALUES],
    x: c_int,
    y: c_int,
    cx: u32,
    cy: u32,
}

/// The recorder's state is one set of C statics: one run at a time.
static RECORDER: Mutex<()> = Mutex::new(());

fn run(original: bool, case: &Case, logs: bool) -> (String, bool) {
    let _recorder = RECORDER.lock().unwrap_or_else(PoisonError::into_inner);
    let mut log = vec![0u8; LOG];
    let (matrix, draw): (MatrixFn, DrawFn) = if original {
        (
            oracle_obs_source_draw_set_color_matrix,
            oracle_obs_source_draw,
        )
    } else {
        (rust_set_color_matrix, rust_draw)
    };
    let (get, set): (Option<GetLog>, Option<SetLog>) = if logs {
        (Some(base_get_log_handler), Some(base_set_log_handler))
    } else {
        (None, None)
    };
    // SAFETY: an 8-int state of 0s and 1s, 24 floats and a LOG-byte buffer;
    // the handler is swapped only in an isolated child.
    let srgb = unsafe {
        oracle_source_draw_run(
            matrix,
            draw,
            case.state.as_ptr(),
            case.values.as_ptr(),
            case.x,
            case.y,
            case.cx,
            case.cy,
            get,
            set,
            log.as_mut_ptr().cast(),
            log.len(),
        )
    };
    let end = log.iter().position(|&b| b == 0).unwrap();
    (String::from_utf8(log[..end].to_vec()).unwrap(), srgb)
}

fn check(case: &Case, logs: bool) {
    let c = run(true, case, logs);
    let rust = run(false, case, logs);
    assert_eq!(
        rust, c,
        "state {:?} at ({}, {}) {}x{}",
        case.state, case.x, case.y, case.cx, case.cy
    );
}

fn values(seed: f32) -> [f32; VALUES] {
    core::array::from_fn(|i| seed + i as f32 * 0.5)
}

fn states() -> impl Iterator<Item = [c_int; STATE]> {
    (0..256).map(state)
}

fn state(bits: c_int) -> [c_int; STATE] {
    core::array::from_fn(|i| (bits >> i) & 1)
}

/// The state logs a line: no current effect, or the NULL matrix or
/// texture. Those run only where the log handler is owned.
fn logs(state: &[c_int; STATE]) -> bool {
    state[1] == 0 || state[5] == 1
}

const POSITIONS: [(c_int, c_int); 9] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (-1, -1),
    (3, 2),
    (i32::MIN, 0),
    (0, i32::MAX),
    (i32::MAX, i32::MIN),
    (16_777_217, -16_777_217),
];

#[test]
fn values_match_c() {
    let mut out = [0 as c_int; 7];
    // SAFETY: the oracle writes seven ints.
    unsafe { oracle_source_draw_values(out.as_mut_ptr()) };
    assert_eq!(out[0], GS_FLIP_V as c_int);
    assert_eq!(out[1], LOG_WARNING);
    assert_eq!(out[2], LOG_DEBUG);
    assert_eq!(out[3], size_of::<Matrix4>() as c_int);
    assert_eq!(out[4], align_of::<Matrix4>() as c_int);
    // struct vec3 is 16 bytes, 16-aligned; the port reads its first 12.
    assert_eq!(out[5], 16);
    assert_eq!(out[6], 16);
}

/// Every state that logs nothing, at every position.
#[test]
fn every_state_matches_c() {
    for state in states().filter(|s| !logs(s)) {
        for (x, y) in POSITIONS {
            check(
                &Case {
                    state,
                    values: values(-2.25),
                    x,
                    y,
                    cx: 7,
                    cy: 0,
                },
                false,
            );
        }
    }
}

/// Every state with the log lines recorded through the real `blog()`, in
/// a child process that owns the log handler.
#[test]
fn every_state_with_logs_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::every_state_with_logs_matches_c")) {
        return;
    }
    let mut logged = 0;
    for state in states() {
        let case = Case {
            state,
            values: values(1.0),
            x: 2,
            y: 0,
            cx: 0,
            cy: 9,
        };
        check(&case, true);
        logged += usize::from(run(true, &case, true).0.contains("log "));
    }
    // The no-effect warnings and the NULL lines both ran.
    assert!(logged > 0);
    let (no_effect, _) = run(
        true,
        &Case {
            state: [1, 0, 0, 0, 0, 0, 0, 0],
            values: values(0.0),
            x: 0,
            y: 0,
            cx: 0,
            cy: 0,
        },
        true,
    );
    assert_eq!(
        no_effect,
        "get_effect;log 200 obs_source_draw: no active effect!;"
    );
    let (null_matrix, _) = run(
        true,
        &Case {
            state: [0, 1, 0, 0, 0, 1, 0, 0],
            values: values(0.0),
            x: 0,
            y: 0,
            cx: 0,
            cy: 0,
        },
        true,
    );
    assert_eq!(
        null_matrix,
        "get_effect;log 400 obs_source_draw_set_color_matrix: Null 'color_matrix' parameter;"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_draws_and_matrices_match_c(
        bits in 0..256 as c_int,
        floats in proptest::collection::vec(any::<u32>(), VALUES),
        x in any::<c_int>(),
        y in any::<c_int>(),
        cx in any::<u32>(),
        cy in any::<u32>(),
    ) {
        // A current effect and a pointer: the states that log nothing.
        let state = state((bits | 0b10) & !0b10_0000);
        prop_assert!(!logs(&state));
        let values = core::array::from_fn(|i| f32::from_bits(floats[i]));
        check(&Case { state, values, x, y, cx, cy }, false);
    }
}
