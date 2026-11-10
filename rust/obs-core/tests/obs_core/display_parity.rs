//! Tier 3 for `obs_core::display`: the shim (`obs_core::ffi::display`) on
//! one stand-in core against the original `libobs/obs-display.c.h` (the
//! oracle's `oracle_obs_display_*` and `oracle_render_display`) on another.
//!
//! Both sides run the same script of creates (with and without graphics
//! data, the swap chain made or refused), destroys, resizes, colour-space
//! flags, callback adds and removes (two functions and four params, so a
//! pair added twice and a remove that matches the param but not the
//! function both occur), enables and renders, each on a display of the
//! script or on NULL, under random host answers (the clear workaround, the
//! sRGB clear, the colour space, the present-ready answer and the solid
//! effect's loop count), changed mid-script. Sizes include values past
//! `INT_MAX` and past 2^24, where `(int)` wraps and `(float)` rounds.
//!
//! After each step the two logs must match: every graphics and libobs call
//! with its arguments (floats as bits), each callback's id, size and the
//! locks it sees held. At the end the list order and links and every live
//! display's fields must match too; then each side destroys what is left.
//!
//! The oracle's core is one global, so the cases run one at a time under
//! [`LOCK`]. Displays are compared by their index in the script, never by
//! address.
//!
//! Intentional difference, excluded: a draw callback with a NULL function
//! (the C calls through NULL and crashes; the shim skips it). The script
//! never adds one.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Mutex;

use obs_c_oracle::display::{self as c, Config};
use obs_core::ffi::display::{self as rs, Calls, DrawFn, vec4};
use proptest::prelude::*;

static LOCK: Mutex<()> = Mutex::new(());

/// The stand-ins the C side's `#define`s name, as the shim's [`Calls`].
enum Rec {}

// SAFETY: each method is the recording stand-in of the C function its trait
// doc names, which only logs and answers from the world's config.
unsafe impl Calls for Rec {
    fn log_error(message: &'static CStr) {
        // SAFETY: a static string, no conversions.
        unsafe { c::oracle_display_blog(100, message.as_ptr()) }
    }
    fn host_clear_workaround() -> bool {
        // SAFETY: logs only.
        unsafe { c::oracle_display_host_clear_workaround() }
    }
    fn host_clear_color_always_srgb() -> bool {
        // SAFETY: logs only.
        unsafe { c::oracle_display_host_clear_color_always_srgb() }
    }
    fn enter_graphics() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_enter_graphics() }
    }
    fn leave_graphics() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_leave_graphics() }
    }
    unsafe fn enter_context(graphics: *mut c_void) {
        // SAFETY: logs the value only.
        unsafe { c::oracle_display_enter_context(graphics) }
    }
    fn leave_context() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_leave_context() }
    }
    unsafe fn swapchain_create(data: *const c_void) -> *mut c_void {
        // SAFETY: the world's graphics data.
        unsafe { c::oracle_display_swapchain_create(data) }
    }
    unsafe fn swapchain_destroy(swap: *mut c_void) {
        // SAFETY: logs the value only.
        unsafe { c::oracle_display_swapchain_destroy(swap) }
    }
    unsafe fn load_swapchain(swap: *mut c_void) {
        // SAFETY: logs the value only.
        unsafe { c::oracle_display_load_swapchain(swap) }
    }
    fn resize(cx: u32, cy: u32) {
        // SAFETY: logs only.
        unsafe { c::oracle_display_resize(cx, cy) }
    }
    fn update_color_space() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_update_color_space() }
    }
    fn is_present_ready() -> bool {
        // SAFETY: logs only.
        unsafe { c::oracle_display_is_present_ready() }
    }
    fn begin_scene() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_begin_scene() }
    }
    fn color_space() -> c_int {
        // SAFETY: logs only.
        unsafe { c::oracle_display_get_color_space() }
    }
    fn clear(flags: u32, color: &vec4, depth: f32, stencil: u8) {
        // SAFETY: reads the vec4 for the call.
        unsafe { c::oracle_display_clear(flags, std::ptr::from_ref(color).cast(), depth, stencil) }
    }
    fn enable_depth_test(enable: bool) {
        // SAFETY: logs only.
        unsafe { c::oracle_display_enable_depth_test(enable) }
    }
    fn set_cull_mode(mode: c_int) {
        // SAFETY: logs only.
        unsafe { c::oracle_display_set_cull_mode(mode) }
    }
    fn ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        // SAFETY: logs only.
        unsafe { c::oracle_display_ortho(left, right, top, bottom, znear, zfar) }
    }
    fn set_viewport(x: c_int, y: c_int, width: c_int, height: c_int) {
        // SAFETY: logs only.
        unsafe { c::oracle_display_set_viewport(x, y, width, height) }
    }
    unsafe fn param_by_name(effect: *mut c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: a static name; the effect is logged only.
        unsafe { c::oracle_display_get_param_by_name(effect, name) }
    }
    unsafe fn effect_set_vec4(param: *mut c_void, value: &vec4) {
        // SAFETY: reads the vec4 for the call.
        unsafe { c::oracle_display_effect_set_vec4(param, std::ptr::from_ref(value).cast()) }
    }
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool {
        // SAFETY: a static name; the effect is logged only.
        unsafe { c::oracle_display_effect_loop(effect, technique) }
    }
    fn draw_sprite(cx: u32, cy: u32) {
        // SAFETY: logs only.
        unsafe { c::oracle_display_draw_sprite(std::ptr::null_mut(), 0, cx, cy) }
    }
    fn end_scene() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_end_scene() }
    }
    fn present() {
        // SAFETY: logs only.
        unsafe { c::oracle_display_present() }
    }
}

/// One step of a script. A display is picked as an index into the ones the
/// script made (destroyed ones are NULL), or NULL when there are none.
#[derive(Debug, Clone)]
enum Op {
    Create {
        data: Option<(u32, u32)>,
        background: u32,
    },
    Destroy(usize),
    Resize(usize, u32, u32),
    UpdateColorSpace(usize),
    Add(usize, bool, c_int),
    Remove(usize, bool, c_int),
    Enable(usize, bool),
    Render(usize),
    Configure(Config),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

fn draw_fn(second: bool) -> DrawFn {
    if second {
        c::oracle_display_draw2
    } else {
        c::oracle_display_draw
    }
}

fn take_log(world: *mut c_void) -> String {
    let mut buf = vec![0u8; 1 << 16];
    // SAFETY: the buffer is writable for its length.
    let len = unsafe { c::oracle_display_take_log(world, buf.as_mut_ptr().cast(), buf.len()) };
    buf.truncate(len);
    String::from_utf8(buf).unwrap()
}

fn state(world: *mut c_void, displays: &[*mut c_void]) -> String {
    let mut buf = vec![0u8; 1 << 16];
    // SAFETY: the displays are live or NULL; the buffer is writable.
    let len = unsafe {
        c::oracle_display_state(
            world,
            displays.as_ptr(),
            displays.len(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    buf.truncate(len);
    String::from_utf8(buf).unwrap()
}

fn pick(displays: &[*mut c_void], i: usize) -> (Option<usize>, *mut c_void) {
    if displays.is_empty() {
        (None, std::ptr::null_mut())
    } else {
        let idx = i % displays.len();
        (Some(idx), displays[idx])
    }
}

/// Runs the script on one side; its transcript.
fn run(side: Side, config: &Config, ops: &[Op]) -> Vec<String> {
    // SAFETY: a new world, used alone under LOCK, freed below.
    let world = unsafe { c::oracle_display_new() };
    // SAFETY: the world is live; the config lives for the call.
    unsafe {
        c::oracle_display_use(world);
        c::oracle_display_configure(world, config);
    }
    let mut displays: Vec<*mut c_void> = Vec::new();
    let mut out = Vec::new();

    for op in ops {
        // SAFETY: every display passed is live (from this side's create) or
        // NULL; destroyed ones are set to NULL at once.
        unsafe {
            match *op {
                Op::Create { data, background } => {
                    let data = data.map_or(std::ptr::null(), |(cx, cy)| {
                        c::oracle_display_data(world, cx, cy)
                    });
                    let d = match side {
                        Side::C => c::oracle_obs_display_create(data, background),
                        Side::Rust => rs::create_with::<Rec>(data, background),
                    };
                    out.push(format!("created {}", !d.is_null()));
                    displays.push(d);
                }
                Op::Destroy(i) => {
                    let (idx, d) = pick(&displays, i);
                    match side {
                        Side::C => c::oracle_obs_display_destroy(d),
                        Side::Rust => rs::destroy_with::<Rec>(d),
                    }
                    if let Some(idx) = idx {
                        displays[idx] = std::ptr::null_mut();
                    }
                }
                Op::Resize(i, cx, cy) => {
                    let (_, d) = pick(&displays, i);
                    match side {
                        Side::C => c::oracle_obs_display_resize(d, cx, cy),
                        Side::Rust => rs::resize_with::<Rec>(d, cx, cy),
                    }
                }
                Op::UpdateColorSpace(i) => {
                    let (_, d) = pick(&displays, i);
                    match side {
                        Side::C => c::oracle_obs_display_update_color_space(d),
                        Side::Rust => rs::update_color_space_with::<Rec>(d),
                    }
                }
                Op::Add(i, second, id) => {
                    let (_, d) = pick(&displays, i);
                    let param = c::oracle_display_param(world, id);
                    let f = Some(draw_fn(second));
                    match side {
                        Side::C => c::oracle_obs_display_add_draw_callback(d, f, param),
                        Side::Rust => rs::add_draw_callback_with::<Rec>(d, f, param),
                    }
                }
                Op::Remove(i, second, id) => {
                    let (_, d) = pick(&displays, i);
                    let param = c::oracle_display_param(world, id);
                    let f = Some(draw_fn(second));
                    match side {
                        Side::C => c::oracle_obs_display_remove_draw_callback(d, f, param),
                        Side::Rust => rs::remove_draw_callback_with::<Rec>(d, f, param),
                    }
                }
                Op::Enable(i, enabled) => {
                    let (_, d) = pick(&displays, i);
                    if !d.is_null() {
                        c::oracle_display_set_enabled(d, enabled);
                    }
                }
                Op::Render(i) => {
                    let (_, d) = pick(&displays, i);
                    c::oracle_display_rendering(world, d);
                    match side {
                        Side::C => c::oracle_render_display(d),
                        Side::Rust => rs::render_with::<Rec>(d),
                    }
                    c::oracle_display_rendering(world, std::ptr::null_mut());
                }
                Op::Configure(next) => c::oracle_display_configure(world, &next),
            }
        }
        out.push(take_log(world));
    }

    out.push(state(world, &displays));
    for d in displays.iter_mut().filter(|d| !d.is_null()) {
        // SAFETY: a live display of this side.
        unsafe {
            match side {
                Side::C => c::oracle_obs_display_destroy(*d),
                Side::Rust => rs::destroy_with::<Rec>(*d),
            }
        }
        *d = std::ptr::null_mut();
    }
    out.push(take_log(world));
    // SAFETY: every display is destroyed; the world is not used again.
    unsafe { c::oracle_display_free(world) };
    out
}

fn check(config: &Config, ops: &[Op]) {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let c_side = run(Side::C, config, ops);
    let rust_side = run(Side::Rust, config, ops);
    assert_eq!(rust_side, c_side, "script {ops:?} under {config:?}");
}

fn size() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(0u32),
        Just(1),
        Just(64),
        Just(1920),
        Just(16_777_217),
        Just(0x8000_0000),
        Just(u32::MAX),
        any::<u32>(),
    ]
}

fn config() -> impl Strategy<Value = Config> {
    (
        any::<bool>(),
        any::<bool>(),
        prop::bool::weighted(0.8),
        prop::bool::weighted(0.15),
        prop_oneof![Just(0), Just(1), Just(2), Just(3), Just(-1), Just(7)],
        0u32..3,
    )
        .prop_map(
            |(workaround, always_srgb, present_ready, swap_fails, space, loops)| Config {
                workaround,
                always_srgb,
                present_ready,
                swap_fails,
                space,
                loops,
            },
        )
}

fn op() -> impl Strategy<Value = Op> {
    let at = 0usize..6;
    prop_oneof![
        3 => (prop::option::weighted(0.7, (size(), size())), any::<u32>())
            .prop_map(|(data, background)| Op::Create { data, background }),
        1 => at.clone().prop_map(Op::Destroy),
        2 => (at.clone(), size(), size()).prop_map(|(i, cx, cy)| Op::Resize(i, cx, cy)),
        1 => at.clone().prop_map(Op::UpdateColorSpace),
        3 => (at.clone(), any::<bool>(), 0..4).prop_map(|(i, s, id)| Op::Add(i, s, id)),
        2 => (at.clone(), any::<bool>(), 0..4).prop_map(|(i, s, id)| Op::Remove(i, s, id)),
        1 => (at.clone(), prop::bool::weighted(0.7)).prop_map(|(i, e)| Op::Enable(i, e)),
        4 => at.prop_map(Op::Render),
        1 => config().prop_map(Op::Configure),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn any_script_matches_the_c(config in config(), ops in prop::collection::vec(op(), 0..24)) {
        check(&config, &ops);
    }
}

fn render_world(config: Config) {
    check(
        &config,
        &[
            Op::Create {
                data: Some((64, 32)),
                background: 0xff33_6699,
            },
            Op::Add(0, false, 0),
            Op::Add(0, true, 1),
            Op::Add(0, false, 0),
            Op::Render(0),
            Op::Resize(0, 200, 100),
            Op::UpdateColorSpace(0),
            Op::Render(0),
            Op::UpdateColorSpace(0),
            Op::Render(0),
            Op::Remove(0, true, 0),
            Op::Remove(0, false, 0),
            Op::Render(0),
        ],
    );
}

#[test]
fn renders_match_under_each_host_answer() {
    for workaround in [false, true] {
        for always_srgb in [false, true] {
            for space in [0, 1, 2, 3] {
                for present_ready in [false, true] {
                    render_world(Config {
                        workaround,
                        always_srgb,
                        present_ready,
                        swap_fails: false,
                        space,
                        loops: 2,
                    });
                }
            }
        }
    }
}

#[test]
fn lists_match_through_creates_and_destroys() {
    let config = Config {
        present_ready: true,
        ..Config::default()
    };
    check(
        &config,
        &[
            Op::Destroy(0),
            Op::Render(0),
            Op::Resize(0, 1, 1),
            Op::UpdateColorSpace(0),
            Op::Add(0, false, 0),
            Op::Remove(0, false, 0),
            Op::Create {
                data: None,
                background: 1,
            },
            Op::Create {
                data: Some((10, 20)),
                background: 2,
            },
            Op::Create {
                data: None,
                background: 3,
            },
            Op::Destroy(1),
            Op::Create {
                data: Some((5, 5)),
                background: 4,
            },
            Op::Destroy(2),
            Op::Destroy(0),
            Op::Configure(Config {
                swap_fails: true,
                ..config
            }),
            Op::Create {
                data: Some((7, 7)),
                background: 5,
            },
            Op::Create {
                data: None,
                background: 6,
            },
        ],
    );
}

#[test]
fn a_disabled_display_matches() {
    check(
        &Config {
            present_ready: true,
            ..Config::default()
        },
        &[
            Op::Create {
                data: Some((64, 32)),
                background: 0,
            },
            Op::Add(0, false, 2),
            Op::Enable(0, false),
            Op::Resize(0, 9, 9),
            Op::UpdateColorSpace(0),
            Op::Render(0),
            Op::Enable(0, true),
            Op::Render(0),
        ],
    );
}

#[test]
fn init_and_free_match() {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut logs = Vec::new();
    for side in [Side::C, Side::Rust] {
        for swap_fails in [false, true] {
            // SAFETY: a new world used alone, a zeroed display of the seam's
            // size, inited, freed and released here.
            unsafe {
                let world = c::oracle_display_new();
                c::oracle_display_use(world);
                c::oracle_display_configure(
                    world,
                    &Config {
                        workaround: true,
                        swap_fails,
                        ..Config::default()
                    },
                );
                let size = c::oracle_display_layout(10);
                let d = obs_util::ffi::darray::bmalloc(size);
                std::ptr::write_bytes(d.cast::<u8>(), 0, size);
                let data = c::oracle_display_data(world, 33, 44);
                let ok = match side {
                    Side::C => c::oracle_obs_display_init(d, data),
                    Side::Rust => rs::init_with::<Rec>(d, data),
                };
                let mut log = format!("{ok} {}", take_log(world));
                if ok {
                    log.push_str(&state(world, &[d]));
                    match side {
                        Side::C => c::oracle_obs_display_free(d),
                        Side::Rust => rs::free_with::<Rec>(d),
                    }
                }
                log.push_str(&take_log(world));
                obs_util::ffi::darray::bfree(d);
                c::oracle_display_free(world);
                logs.push(log);
            }
        }
    }
    assert_eq!(logs[2..], logs[..2]);
}
