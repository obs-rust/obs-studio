//! Tier 1 mirrors test_scene_item_crop_setter.c: the raw-input compare,
//! the clamp, the store decision and the null order.
use obs_scene::item_crop_setter::{Missing, clamp, store, validate};

fn set(stored: &mut [i32; 4], input: [i32; 4]) -> bool {
    store(stored, input, clamp(input))
}

#[test]
fn null_arguments_check_item_first() {
    assert_eq!(validate(true, true), Ok(()));
    assert_eq!(validate(false, true), Err(Missing::Item));
    assert_eq!(validate(false, false), Err(Missing::Item));
    assert_eq!(validate(true, false), Err(Missing::Crop));
}

#[test]
fn clamp_zeroes_only_negative_sides() {
    assert_eq!(clamp([11, 22, 33, 44]), [11, 22, 33, 44]);
    assert_eq!(clamp([-1, 2, -3, 4]), [0, 2, 0, 4]);
    assert_eq!(clamp([i32::MIN, i32::MAX, 0, -1]), [0, i32::MAX, 0, 0]);
}

#[test]
fn positive_input_and_equal_noop() {
    let crop = [11, 33, 22, 44];
    let mut stored = [0; 4];
    assert!(set(&mut stored, crop));
    assert_eq!(stored, crop);
    assert!(!set(&mut stored, crop));
    assert_eq!(stored, crop);
}

#[test]
fn raw_negative_input_stores_every_time() {
    let inputs = [
        [-1, 3, 2, 4],
        [1, 3, -2, 4],
        [1, -3, 2, 4],
        [1, 3, 2, -4],
        [i32::MIN; 4],
    ];
    let expected = [
        [0, 3, 2, 4],
        [1, 3, 0, 4],
        [1, 0, 2, 4],
        [1, 3, 2, 0],
        [0; 4],
    ];
    for (input, clamped) in inputs.into_iter().zip(expected) {
        let mut stored = clamped;
        for _ in 0..3 {
            assert!(set(&mut stored, input), "{input:?}");
            assert_eq!(stored, clamped);
        }
        assert!(!set(&mut stored, clamped));
        assert_eq!(stored, clamped);
    }
}

#[test]
fn store_compares_raw_input_not_clamped() {
    let mut stored = [7, 7, 7, 7];
    // A clamped crop equal to the stored one still stores when the raw differs.
    assert!(store(&mut stored, [-1, 7, 7, 7], [7, 7, 7, 7]));
    assert_eq!(stored, [7, 7, 7, 7]);
    // A raw input equal to the stored one stores nothing, whatever the clamp.
    assert!(!store(&mut stored, [7, 7, 7, 7], [0, 0, 0, 0]));
    assert_eq!(stored, [7, 7, 7, 7]);
}
