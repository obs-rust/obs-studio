//! Tier 3: exact callback stream from the original C and the private Rust ABI.
use super::video_render_size::{Call, Recorder};
use obs_c_oracle::video_render_size::oracle_obs_video_set_render_size;
use obs_core::ffi::video_render_size::obs_video_set_render_size;
use obs_core::video_render_size::set_render_size;
use proptest::prelude::*;
use std::cell::RefCell;

thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
unsafe extern "C" fn depth(v: bool) {
    CALLS.with_borrow_mut(|c| c.push(Call::Depth(v)));
}
unsafe extern "C" fn cull(v: i32) {
    CALLS.with_borrow_mut(|c| c.push(Call::Cull(v)));
}
unsafe extern "C" fn ortho(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
    CALLS.with_borrow_mut(|v| v.push(Call::Ortho([a, b, c, d, e, f].map(f32::to_bits))));
}
unsafe extern "C" fn viewport(a: i32, b: i32, c: i32, d: i32) {
    CALLS.with_borrow_mut(|v| v.push(Call::Viewport([a, b, c, d])));
}
fn check(width: u32, height: u32) {
    CALLS.with_borrow_mut(Vec::clear);
    // SAFETY: thread-local recorders are valid, non-unwinding callbacks.
    unsafe {
        oracle_obs_video_set_render_size(width, height, depth, cull, ortho, viewport);
    }
    let expected = CALLS.with_borrow_mut(std::mem::take);
    // SAFETY: same callbacks and invocation lifetime as the oracle above.
    unsafe {
        obs_video_set_render_size(width, height, depth, cull, ortho, viewport);
    }
    assert_eq!(CALLS.with_borrow_mut(std::mem::take), expected);
    let mut recorder = Recorder::default();
    set_render_size(width, height, &mut recorder);
    assert_eq!(recorder.0, expected);
}
#[test]
fn rounding_and_signed_conversion_edges() {
    let edges = [
        0,
        1,
        1080,
        1920,
        16777215,
        16777216,
        16777217,
        i32::MAX as u32,
        1 << 31,
        u32::MAX,
    ];
    for width in edges {
        for height in edges {
            check(width, height);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1024)))]
    #[test]
    fn arbitrary_dimensions(width in any::<u32>(), height in any::<u32>()) {
        check(width,height);
    }
}
