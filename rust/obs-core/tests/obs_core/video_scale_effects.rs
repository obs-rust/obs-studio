//! Tier 1 mirrors test_video_scale_effect_pointers.c, including absent selections.
use obs_core::video_scale_effects::{loaded, selected};

#[test]
fn all_effect_presence_masks() {
    for mask in 0..32 {
        let effects = core::array::from_fn(|i| (mask & (1 << i) != 0).then_some(i + 1));
        assert_eq!(loaded(&effects), mask);
        for selector in 0..5 {
            assert_eq!(selected(&effects, selector), effects[selector as usize]);
        }
        for unknown in [i32::MIN, -1, 5, 99, i32::MAX] {
            assert_eq!(selected(&effects, unknown), effects[0]);
        }
        assert_eq!(
            effects,
            core::array::from_fn(|i| (mask & (1 << i) != 0).then_some(i + 1))
        );
    }
}
