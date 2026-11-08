//! Exact real-layout parity with the original C queries; no input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_scene::ffi::item_transition_getters::{
    Fields, obs_sceneitem_get_transition, obs_sceneitem_get_transition_duration,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_scene_item_transition_layout(layout: *mut usize);
    fn oracle_scene_item_transition_new(
        shown: u32,
        hidden: u32,
        has_show: bool,
        has_hide: bool,
    ) -> *mut c_void;
    fn oracle_scene_item_transition_unchanged(item: *const c_void) -> bool;
    fn oracle_scene_item_transition_free(item: *mut c_void);
    fn oracle_obs_sceneitem_get_transition(item: *mut c_void, show: bool) -> *mut c_void;
    fn oracle_obs_sceneitem_get_transition_duration(item: *mut c_void, show: bool) -> u32;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the oracle writes exactly eight layout measurements.
    unsafe { oracle_scene_item_transition_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, show_transition),
            offset_of!(Fields, hide_transition),
            offset_of!(Fields, show_duration),
            offset_of!(Fields, hide_duration),
            size_of::<u32>(),
            align_of::<u32>()
        ]
    );
}

fn check(shown: u32, hidden: u32, has_show: bool, has_hide: bool) {
    // SAFETY: the oracle allocates a real item with live source sentinels,
    // which remains readable until the paired free. Neither getter mutates it.
    unsafe {
        let item = oracle_scene_item_transition_new(shown, hidden, has_show, has_hide);
        assert!(!item.is_null());
        for _ in 0..2 {
            for show in [false, true] {
                let pointer = obs_sceneitem_get_transition(item, show);
                assert_eq!(pointer, oracle_obs_sceneitem_get_transition(item, show));
                assert_eq!(!pointer.is_null(), if show { has_show } else { has_hide });
                let duration = obs_sceneitem_get_transition_duration(item, show);
                assert_eq!(
                    duration,
                    oracle_obs_sceneitem_get_transition_duration(item, show)
                );
                assert_eq!(duration, if show { shown } else { hidden });
                assert!(oracle_scene_item_transition_unchanged(item));
            }
        }
        oracle_scene_item_transition_free(item);
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: both original getter contracts explicitly accept null items.
    unsafe {
        for show in [false, true] {
            let item = core::ptr::null_mut();
            assert_eq!(
                obs_sceneitem_get_transition(item, show),
                oracle_obs_sceneitem_get_transition(item, show)
            );
            assert!(obs_sceneitem_get_transition(item, show).is_null());
            assert_eq!(
                obs_sceneitem_get_transition_duration(item, show),
                oracle_obs_sceneitem_get_transition_duration(item, show)
            );
            assert_eq!(obs_sceneitem_get_transition_duration(item, show), 0);
        }
    }
    for shown in [0, 1, 250, u32::MAX] {
        for hidden in [0, 1, 250, u32::MAX] {
            for has_show in [false, true] {
                for has_hide in [false, true] {
                    check(shown, hidden, has_show, has_hide);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn getters_match_c(shown in any::<u32>(), hidden in any::<u32>(), has_show in any::<bool>(), has_hide in any::<bool>()) {
        check(shown, hidden, has_show, has_hide);
    }
}
