//! Tier 1 mirrors the value/null assertions of test_scene_item_crop_getter.c.
use obs_scene::item_crop_getter::{Missing, get};

#[test]
fn null_arguments() {
    let mut output = [1, 2, 3, 4];
    assert_eq!(get(None, Some(&mut output)), Err(Missing::Item));
    assert_eq!(output, [1, 2, 3, 4]);
    assert_eq!(get(Some([0; 4]), None), Err(Missing::Crop));
    assert_eq!(get(None, None), Err(Missing::Item));
}

#[test]
fn stored_crop() {
    let edges = [0, -1, 1, i32::MIN, i32::MAX];
    for i in 0..edges.len() {
        let value = [
            edges[i],
            edges[(i + 1) % 5],
            edges[(i + 2) % 5],
            edges[(i + 3) % 5],
        ];
        let mut output = [0; 4];
        assert_eq!(get(Some(value), Some(&mut output)), Ok(()));
        assert_eq!(output, value);
    }
}
