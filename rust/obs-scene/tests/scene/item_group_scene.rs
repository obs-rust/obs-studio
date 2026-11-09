//! Tier 1 mirrors test_scene_item_group_scene.c with safe identity tokens.
use obs_scene::item_group_scene::group_scene;

#[test]
fn item_group_scene() {
    assert_eq!(group_scene::<usize>(None, || panic!("absent item read")), 0);
    for bits in 0..32 {
        let is_group = bits & 1 != 0;
        let scene = if bits & 8 != 0 { 1 + ((bits >> 4) & 1) } else { 0 };
        for _ in 0..2 {
            assert_eq!(
                group_scene(Some(is_group), || {
                    assert!(is_group, "non-group source read");
                    scene
                }),
                if is_group { scene } else { 0 }
            );
        }
    }
}
