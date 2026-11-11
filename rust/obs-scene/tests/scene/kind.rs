//! Safe mirrors of test_scene_kind.c; object creation/lifetime stays in Tier 2.
use obs_scene::kind::{is_group, is_scene};

fn assert_kind(id: Option<&[u8]>, scene: bool, group: bool) {
    for _ in 0..3 {
        assert_eq!(is_scene(id), scene);
        assert_eq!(is_group(id), group);
    }
}

#[test]
fn test_scene_kind_null_source() {
    // Before, during and after core lifetime: classification has no global state.
    for _ in 0..3 {
        assert_kind(None, false, false);
    }
}

#[test]
fn test_scene_kind_public_scene() {
    assert_kind(Some(b"scene"), true, false);
}

#[test]
fn test_scene_kind_private_scene() {
    assert_kind(Some(b"scene"), true, false);
}

#[test]
fn test_scene_kind_group_source() {
    assert_kind(Some(b"group"), false, true);
}

#[test]
fn test_scene_kind_plain_inputs() {
    for id in [
        b"kind-input".as_slice(),
        b"scene-extra",
        b"group-extra",
        b"Scene",
        b"Group",
    ] {
        assert_kind(Some(id), false, false);
    }
}

#[test]
fn test_scene_kind_identifier_only() {
    // The safe API cannot inspect context or type. Mutable backing arrays must
    // match by value; the shim supplies bytes up to the first C terminator.
    for (id, scene, group) in [
        (b"scene".to_vec(), true, false),
        (b"group".to_vec(), false, true),
    ] {
        assert_kind(Some(&id), scene, group);
    }
    assert_kind(Some(b"plain"), false, false);
    assert_kind(Some(b""), false, false);
    for (id, scene, group) in [
        (b"scene\0suffix".as_slice(), true, false),
        (b"group\0suffix", false, true),
    ] {
        let end = id.iter().position(|&byte| byte == 0).unwrap();
        assert_kind(Some(&id[..end]), scene, group);
    }
    assert_kind(Some(b"scen"), false, false);
    assert_kind(Some(b"grou"), false, false);
    assert_kind(Some(b"\xff"), false, false);
}
