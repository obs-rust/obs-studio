//! GPU-free Tier 3 against the original canvas AND view C render bodies.
//! Reuses the view renderer's adapters/doubles; NULL canvas is C UB, excluded.
//! Real graphics/context/reference assertions stay in unchanged Tier 2.
use core::ffi::{c_char, c_longlong, c_void};
use obs_c_oracle::view::oracle_view_set_trace;
use obs_core::ffi::canvas_channel_getter::Fields;
use obs_core::ffi::canvas_render::obs_canvas_render;
use proptest::prelude::*;
use std::cell::RefCell;
use std::ffi::CStr;

unsafe extern "C" {
    fn oracle_obs_canvas_render(canvas: *mut c_void);
    fn oracle_canvas_render_new() -> *mut c_void;
    fn oracle_canvas_render_free(data: *mut c_void);
    fn oracle_canvas_render_reset(data: *mut c_void, channels: *const u8, removed: u32, flags: u32);
    fn oracle_canvas_render_canvas(data: *mut c_void, selected: u32) -> *mut c_void;
    fn oracle_canvas_render_view(data: *mut c_void, selected: u32) -> *mut c_void;
    fn oracle_canvas_render_effect(data: *mut c_void, selected: u32, change: bool);
    fn oracle_canvas_render_size() -> usize;
    fn oracle_canvas_render_snapshot(data: *mut c_void, bytes: *mut u8);
    fn obs_canvas_channel_fields(canvas: *mut c_void) -> Fields;
}

#[derive(Default)]
struct State {
    fixture: *mut c_void,
    selected: u32,
    change: bool,
    graphics: bool,
    linear_srgb: bool,
    trace: Vec<(String, String, c_longlong)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

unsafe extern "C" fn record(event: *const c_char, name: *const c_char, value: c_longlong) {
    // SAFETY: view oracle passes static NUL-terminated event and source names.
    let event = unsafe { CStr::from_ptr(event) }.to_str().unwrap();
    let name = if name.is_null() {
        ""
    } else {
        // SAFETY: as above.
        unsafe { CStr::from_ptr(name) }.to_str().unwrap()
    };
    STATE.with_borrow_mut(|state| {
        state.trace.push((event.to_owned(), name.to_owned(), value));
        if event == "render" {
            // SAFETY: this callback runs synchronously on the live C fixture.
            unsafe {
                oracle_canvas_render_effect(
                    state.fixture,
                    state.selected,
                    state.change && name == "a",
                )
            };
            if state.graphics {
                state.linear_srgb = false;
            }
        }
    });
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the test holds the view oracle lock and owns this allocation.
        unsafe {
            oracle_view_set_trace(None);
            oracle_canvas_render_free(self.0);
        }
        STATE.with_borrow_mut(|s| *s = State::default());
    }
}

fn check(
    channels: &[u8; 128],
    selected: u32,
    removed: u32,
    flags: u32,
    change: bool,
    graphics: bool,
) -> Vec<(String, String, c_longlong)> {
    let _lock = super::view_parity::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // SAFETY: C allocates/owns its actual-header fixture; no graphics, threads or
    // pthread calls. Both runs reuse EXACT addresses and reset every state byte.
    unsafe {
        let fixture = Fixture(oracle_canvas_render_new());
        assert!(!fixture.0.is_null());
        let mut results = Vec::new();
        for renderer in [oracle_obs_canvas_render, obs_canvas_render] {
            oracle_canvas_render_reset(fixture.0, channels.as_ptr(), removed, flags);
            let canvas = oracle_canvas_render_canvas(fixture.0, selected);
            assert_eq!(
                obs_canvas_channel_fields(canvas).view,
                oracle_canvas_render_view(fixture.0, selected)
            );
            STATE.with_borrow_mut(|state| {
                *state = State {
                    fixture: fixture.0,
                    selected,
                    change,
                    graphics,
                    linear_srgb: true,
                    trace: Vec::new(),
                }
            });
            oracle_view_set_trace(Some(record));
            renderer(canvas);
            oracle_view_set_trace(None);
            let mut bytes = vec![0; oracle_canvas_render_size()];
            oracle_canvas_render_snapshot(fixture.0, bytes.as_mut_ptr());
            let (trace, srgb) = STATE
                .with_borrow_mut(|state| (std::mem::take(&mut state.trace), state.linear_srgb));
            results.push((trace, srgb, bytes));
        }
        assert_eq!(
            results[0], results[1],
            "canvas/view rendering trace or fixture state differs"
        );
        results.swap_remove(0).0
    }
}

#[test]
fn characterized_empty_view() {
    for selected in [0, 1] {
        for graphics in [false, true] {
            let mut channels = [0; 128];
            // A distinct other canvas must not be rendered, even for empty views.
            channels[(1 - selected as usize) * 64] = 3;
            assert_eq!(
                check(&channels, selected, 0, u32::MAX, false, graphics),
                [
                    ("lock".into(), "".into(), 0),
                    ("unlock".into(), "".into(), 0),
                ]
            );
        }
    }
}

#[test]
fn characterized_delegation() {
    for selected in [0, 1] {
        for graphics in [false, true] {
            let mut channels = [0; 128];
            let start = selected as usize * 64;
            channels[start] = 1;
            channels[start + 2] = 2;
            channels[start + 3] = 1;
            channels[(1 - selected as usize) * 64] = 3;
            assert_eq!(
                check(&channels, selected, 0, 16, true, graphics),
                [
                    ("lock".into(), "".into(), 0),
                    ("render".into(), "a".into(), 0),
                    ("render".into(), "b".into(), 0),
                    ("render".into(), "a".into(), 0),
                    ("render".into(), "b".into(), 0),
                    ("unlock".into(), "".into(), 0),
                ]
            );
        }
    }
    for removed in 0..8 {
        check(&[1; 128], 1, removed, u32::MAX, true, false);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn canvas_render_matches_c(channels in prop::array::uniform::<_, 128>(0u8..4), selected in 0u32..2,
        removed in 0u32..8, flags in any::<u32>(), change in any::<bool>(), graphics in any::<bool>()) {
        check(&channels, selected, removed, flags, change, graphics);
    }
}
