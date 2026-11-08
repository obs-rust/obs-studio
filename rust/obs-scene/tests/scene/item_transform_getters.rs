//! Tier 1 mirrors test_scene_item_transform_getters.c scalar assertions.
use obs_scene::item_transform_getters::{alignment, rotation};

#[test]
fn null_item() {
    assert_eq!(rotation(None).to_bits(), 0);
    assert_eq!(alignment(None), 0);
}

#[test]
fn stored_transform_fields() {
    for (i, bits) in [
        0,
        0x8000_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc1_2345,
        0xffc5_4321,
        1,
        0x7f7f_ffff,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(rotation(Some(f32::from_bits(bits))).to_bits(), bits);
        let value = u32::MAX - i as u32;
        assert_eq!(alignment(Some(value)), value);
    }
    assert_eq!(rotation(Some(-0.0)).to_bits(), 0x8000_0000);
    assert_eq!(alignment(Some(u32::MAX)), u32::MAX);
}
