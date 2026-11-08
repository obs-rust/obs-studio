//! Tier 1 mirror of test_scene_group_queries.c.
use obs_scene::group_queries::is_group;

#[test]
fn group_queries_characterization() {
    assert!(!is_group(None));
    for bits in 0..16 {
        let item = bits & 1 != 0;
        let scene = bits & 2 != 0;
        assert_eq!(is_group(Some(&item)), item);
        assert_eq!(is_group(Some(&scene)), scene);
    }
}
