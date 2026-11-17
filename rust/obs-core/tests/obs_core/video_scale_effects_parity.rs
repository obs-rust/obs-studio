//! Tier 2 C caller contract and Tier 3 pointer/state parity. No exclusions.
use core::ffi::c_void;
use obs_core::ffi::video_scale_effects::{
    obs_video_loaded_scale_effects, obs_video_scale_effect_pointer,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_video_scale_effect_new(mask: u32) -> *mut c_void;
    fn oracle_video_scale_effect_loaded(video: *const c_void) -> u32;
    fn oracle_video_scale_effect_selected(video: *const c_void, selector: i32) -> *mut c_void;
    fn oracle_video_scale_effect_unchanged(video: *const c_void) -> bool;
    fn oracle_video_scale_effect_free(video: *mut c_void);
    fn video_scale_effects_caller_check() -> i32;
}

fn check(mask: u32, selector: i32) {
    // SAFETY: C owns the real-layout video as the fixture's first member until
    // free; both implementations only borrow its fields and effect identities.
    unsafe {
        let video = oracle_video_scale_effect_new(mask);
        assert!(!video.is_null());
        assert_eq!(
            obs_video_loaded_scale_effects(video),
            oracle_video_scale_effect_loaded(video)
        );
        assert_eq!(
            obs_video_scale_effect_pointer(video, selector),
            oracle_video_scale_effect_selected(video, selector)
        );
        assert!(oracle_video_scale_effect_unchanged(video));
        oracle_video_scale_effect_free(video);
    }
}

#[test]
fn characterized_edges() {
    for mask in 0..32 {
        for selector in [i32::MIN, -1, 0, 1, 2, 3, 4, 5, 99, i32::MAX] {
            check(mask, selector);
        }
    }
}

#[test]
fn real_c_caller_reaches_rust_dispatch() {
    // SAFETY: the C test owns its fixture and never opens graphics or starts threads.
    assert_eq!(unsafe { video_scale_effects_caller_check() }, 0);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn pointer_and_state_match_c(mask in 0u32..32, selector in any::<i32>()) {
        check(mask, selector);
    }
}
