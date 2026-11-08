//! Tier 1 mirror of test_scene_source_getters.c, using safe source tokens.
use obs_scene::source_getters::source;

#[test]
fn borrowed_source_characterization() {
    assert_eq!(source::<usize>(None), 0);
    assert_eq!(source(Some(0usize)), 0);
    let scene_source = 17usize;
    let item_source = scene_source;
    let before = (scene_source, item_source);
    assert_eq!(source(Some(scene_source)), scene_source);
    assert_eq!(source(Some(item_source)), item_source);
    assert_eq!(source(Some(scene_source)), source(Some(item_source)));
    assert_eq!((scene_source, item_source), before);
}

#[test]
fn source_tokens_are_independent() {
    for stored in [0, 1, usize::MAX / 2, usize::MAX] {
        assert_eq!(source(Some(stored)), stored);
    }
}
