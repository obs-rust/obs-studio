//! Tier 1 mirror of test_scene_source_type_queries.c.
use obs_scene::source_type_queries::{is_group, is_scene};

#[test]
fn source_type_queries() {
    assert!(!is_group(None));
    assert!(!is_scene(None));
    let ids: &[&[u8]] = &[
        b"", b"group", b"scene", b"Group", b"Scene", b"grou", b"scen", b"groups", b"scenes",
        b" scene", b"group ", b"\xff", b"group", b"scene",
    ];
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(is_group(Some(id)), i == 1 || i == 12);
        assert_eq!(is_scene(Some(id)), i == 2 || i == 13);
    }
}
