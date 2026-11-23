//! Tier 3: the unchanged original C (`libobs/obs-render-canvas-texture.c.h`,
//! oracle `oracle_obs_render_*`) versus the production Rust bridge/core
//! (`render_main_with` / `render_canvas_with`) over one real-header core,
//! three canvases and two mixes, read through the real field adapters, with
//! one recorder for every libobs and graphics call
//! (`oracle/render_canvas_texture.c`).
//!
//! Both sides must leave the same log: each call with its arguments
//! (pointers as fixture ids, the multiplier as bits, the technique and param
//! names), and leave the core and canvases (and, without clobbering, the
//! mixes) unchanged. With clobbering, the recorder gives every mix another
//! render space and texture on each call, so a field read moved past a call
//! changes the log.
//!
//! Over every entry point x main canvas x canvas argument x each canvas's mix
//! (none, 0, 1) x each mix's texture_rendered x four device colour spaces x
//! the sRGB flag x clobbering, then 512 proptest cases over every field,
//! the colour spaces (one past the enum), the pass count (0..3), NULL
//! effect/params and any white level bits.
//!
//! Excluded, the one intentional difference: a NULL canvas or main canvas
//! (the C reads `canvas->mix` through NULL and crashes; the Rust draws
//! nothing, covered by Tier 1). No uninitialized-core cases (C UB).
use core::ffi::{c_char, c_int, c_void};
use obs_core::ffi::render_canvas_texture::{Calls, render_canvas_with, render_main_with};
use obs_core::render_canvas_texture::{
    GS_BLEND_INVSRCALPHA, GS_BLEND_ONE, GS_BLEND_ZERO, OBS_EFFECT_DEFAULT,
};
use proptest::prelude::*;

const LOG: usize = 4096;
const STATE: usize = 18;

type MainFn = unsafe extern "C" fn();
type CanvasFn = unsafe extern "C" fn(*mut c_void);

unsafe extern "C" {
    fn oracle_obs_render_main_texture();
    fn oracle_obs_render_main_texture_src_color_only();
    fn oracle_obs_render_canvas_texture(canvas: *mut c_void);
    fn oracle_obs_render_canvas_texture_src_color_only(canvas: *mut c_void);
    fn oracle_render_canvas_texture_run(
        main_over: MainFn,
        main_src: MainFn,
        canvas_over: CanvasFn,
        canvas_src: CanvasFn,
        state: *const c_int,
        white: f32,
        log: *mut c_char,
        log_len: usize,
        report: *mut c_int,
    );
    fn oracle_render_canvas_texture_values(out: *mut c_int);

    fn obs_render_canvas_texture_main_canvas() -> *const c_void;
    fn obs_render_canvas_texture_mix(canvas: *const c_void) -> *const c_void;
    fn obs_render_canvas_texture_rendered(mix: *const c_void) -> bool;
    fn obs_render_canvas_texture_space(mix: *const c_void) -> c_int;
    fn obs_render_canvas_texture_texture(mix: *const c_void) -> *mut c_void;

    fn oracle_rct_color_space() -> c_int;
    fn oracle_rct_white_level() -> f32;
    fn oracle_rct_srgb_enabled() -> bool;
    fn oracle_rct_enable_srgb(enable: bool);
    fn oracle_rct_base_effect(effect: c_int) -> *mut c_void;
    fn oracle_rct_param(effect: *const c_void, name: *const c_char) -> *mut c_void;
    fn oracle_rct_set_texture_srgb(param: *mut c_void, texture: *mut c_void);
    fn oracle_rct_set_float(param: *mut c_void, value: f32);
    fn oracle_rct_blend_push();
    fn oracle_rct_blend_pop();
    fn oracle_rct_blend_function(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32);
    fn oracle_rct_effect_loop(effect: *mut c_void, technique: *const c_char) -> bool;
    fn oracle_rct_draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32);
}

/// The C side's real adapters and recording stand-ins, as the shim's
/// [`Calls`].
enum Rec {}

// SAFETY: each method is the real adapter over the fixture or the recording
// stand-in of the C function its trait doc names, which only logs, changes
// the fixture's mixes when asked, and answers from the fixture's config.
unsafe impl Calls for Rec {
    fn main_canvas() -> *const c_void {
        // SAFETY: the fixture's core, live for the run.
        unsafe { obs_render_canvas_texture_main_canvas() }
    }
    unsafe fn mix(canvas: *const c_void) -> *const c_void {
        // SAFETY: a fixture canvas.
        unsafe { obs_render_canvas_texture_mix(canvas) }
    }
    unsafe fn rendered(mix: *const c_void) -> bool {
        // SAFETY: a fixture mix.
        unsafe { obs_render_canvas_texture_rendered(mix) }
    }
    unsafe fn space(mix: *const c_void) -> c_int {
        // SAFETY: a fixture mix.
        unsafe { obs_render_canvas_texture_space(mix) }
    }
    unsafe fn texture(mix: *const c_void) -> *mut c_void {
        // SAFETY: a fixture mix.
        unsafe { obs_render_canvas_texture_texture(mix) }
    }
    fn color_space() -> c_int {
        // SAFETY: logs only.
        unsafe { oracle_rct_color_space() }
    }
    fn sdr_white_level() -> f32 {
        // SAFETY: logs only.
        unsafe { oracle_rct_white_level() }
    }
    fn framebuffer_srgb_enabled() -> bool {
        // SAFETY: logs only.
        unsafe { oracle_rct_srgb_enabled() }
    }
    fn enable_framebuffer_srgb(enable: bool) {
        // SAFETY: logs only.
        unsafe { oracle_rct_enable_srgb(enable) }
    }
    fn base_effect(effect: c_int) -> *mut c_void {
        // SAFETY: logs only.
        unsafe { oracle_rct_base_effect(effect) }
    }
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: logs the id and the static name.
        unsafe { oracle_rct_param(effect, name) }
    }
    unsafe fn set_texture_srgb(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: logs ids only.
        unsafe { oracle_rct_set_texture_srgb(param, texture) }
    }
    unsafe fn set_float(param: *mut c_void, value: f32) {
        // SAFETY: logs only.
        unsafe { oracle_rct_set_float(param, value) }
    }
    fn blend_state_push() {
        // SAFETY: logs only.
        unsafe { oracle_rct_blend_push() }
    }
    fn blend_function_separate(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
        // SAFETY: logs only.
        unsafe { oracle_rct_blend_function(src_c, dest_c, src_a, dest_a) }
    }
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool {
        // SAFETY: logs the id and the static name.
        unsafe { oracle_rct_effect_loop(effect, technique) }
    }
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32) {
        // SAFETY: logs ids only.
        unsafe { oracle_rct_draw_sprite(texture, flip, width, height) }
    }
    fn blend_state_pop() {
        // SAFETY: logs only.
        unsafe { oracle_rct_blend_pop() }
    }
}

unsafe extern "C" fn rust_main_over() {
    // SAFETY: the fixture's live core, canvases and mixes; the recorder
    // never dereferences a texture, effect or param.
    unsafe { render_main_with::<Rec>(false) }
}

unsafe extern "C" fn rust_main_src() {
    // SAFETY: as in rust_main_over.
    unsafe { render_main_with::<Rec>(true) }
}

unsafe extern "C" fn rust_canvas_over(canvas: *mut c_void) {
    // SAFETY: as in rust_main_over, for a fixture canvas.
    unsafe { render_canvas_with::<Rec>(canvas, false) }
}

unsafe extern "C" fn rust_canvas_src(canvas: *mut c_void) {
    // SAFETY: as in rust_canvas_over.
    unsafe { render_canvas_with::<Rec>(canvas, true) }
}

fn run(original: bool, state: &[c_int; STATE], white: f32) -> (String, [c_int; 2]) {
    let mut log = vec![0u8; LOG];
    let mut report = [0 as c_int; 2];
    let (main_over, main_src, canvas_over, canvas_src): (MainFn, MainFn, CanvasFn, CanvasFn) =
        if original {
            (
                oracle_obs_render_main_texture,
                oracle_obs_render_main_texture_src_color_only,
                oracle_obs_render_canvas_texture,
                oracle_obs_render_canvas_texture_src_color_only,
            )
        } else {
            (
                rust_main_over,
                rust_main_src,
                rust_canvas_over,
                rust_canvas_src,
            )
        };
    // SAFETY: an 18-int state in the fixture's ranges, a LOG-byte buffer and
    // a 2-int report. The fixture resets its owned core after each run.
    unsafe {
        oracle_render_canvas_texture_run(
            main_over,
            main_src,
            canvas_over,
            canvas_src,
            state.as_ptr(),
            white,
            log.as_mut_ptr().cast(),
            LOG,
            report.as_mut_ptr(),
        )
    };
    let len = log.iter().position(|&b| b == 0).unwrap();
    log.truncate(len);
    (String::from_utf8(log).unwrap(), report)
}

fn compare(state: &[c_int; STATE], white: f32) -> String {
    let c = run(true, state, white);
    let rust = run(false, state, white);
    let at = format!("state={state:?} white={:08x}", white.to_bits());
    assert_eq!(rust.0, c.0, "{at}");
    assert_eq!(c.1, [1, 1], "the C changed the fixture: {at}");
    assert_eq!(rust.1, [1, 1], "the Rust changed the fixture: {at}");
    c.0
}

/// The real-header enum values the port names.
fn values_match() {
    let mut values = [0 as c_int; 6];
    // SAFETY: the probe writes exactly six values.
    unsafe { oracle_render_canvas_texture_values(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [
            GS_BLEND_ZERO as c_int,
            GS_BLEND_ONE as c_int,
            GS_BLEND_INVSRCALPHA as c_int,
            OBS_EFFECT_DEFAULT,
            size_of::<u32>() as c_int,
            size_of::<c_int>() as c_int,
        ]
    );
}

/// Every entry point x main canvas x canvas argument x each canvas's mix x
/// each mix's flag x device colour space x sRGB flag x clobbering, with the
/// mixes in different spaces (709 extended and sRGB) and textures.
fn grid_matches() {
    let mut i = 0u64;
    let mut drew = 0usize;
    for entry in 0..4 {
        for main in 0..3 {
            for canvas in 0..3 {
                for mixes in 0..27 {
                    for rendered in 0..4 {
                        for color_space in 0..4 {
                            for flags in 0..4 {
                                i += 1;
                                if !obs_c_oracle::parity::keep(i) {
                                    continue;
                                }
                                let state: [c_int; STATE] = [
                                    entry,
                                    main,
                                    canvas,
                                    mixes % 3 - 1,
                                    mixes / 3 % 3 - 1,
                                    mixes / 9 - 1,
                                    rendered & 1,
                                    rendered >> 1,
                                    2,
                                    0,
                                    3,
                                    6,
                                    color_space,
                                    flags & 1,
                                    1,
                                    0,
                                    0,
                                    flags >> 1,
                                ];
                                if compare(&state, 300.0).contains("draw_sprite") {
                                    drew += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(drew > 0, "no grid case drew");
}

/// Every field at random, a colour space one past the enum, 0..3 passes,
/// NULL effect and params, and any white level bits.
fn random_states_match() {
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                (0..4, 0..3, 0..3),
                prop::array::uniform3(-1..2),
                prop::array::uniform2(0..2),
                prop::array::uniform2(0..5),
                prop::array::uniform2(-1..8),
                (0..5, 0..2, 0..4, 0..2, 0..2, 0..2),
                any::<u32>(),
            ),
            |((entry, main, canvas), mixes, rendered, spaces, textures, rest, white)| {
                let (color_space, srgb, passes, null_effect, null_params, clobber) = rest;
                let state: [c_int; STATE] = [
                    entry,
                    main,
                    canvas,
                    mixes[0],
                    mixes[1],
                    mixes[2],
                    rendered[0],
                    rendered[1],
                    spaces[0],
                    spaces[1],
                    textures[0],
                    textures[1],
                    color_space,
                    srgb,
                    passes,
                    null_effect,
                    null_params,
                    clobber,
                ];
                compare(&state, f32::from_bits(white));
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn original_c_calls_match() {
    // A todo!() or panic inside the extern "C" bridge aborts the process,
    // so the cases run in a child.
    const CHILD: &str = "OBS_RENDER_CANVAS_TEXTURE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::original_c_calls_match",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    values_match();
    grid_matches();
    random_states_match();
}
