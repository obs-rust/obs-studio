//! Tier 1 mirrors every case of unchanged test_scene_group_enum.c.
use obs_scene::{group_enum::enum_group_items, item_group_scene::group_scene};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Scene {
    id: usize,
    items: &'static [usize],
}

#[test]
fn null_and_non_group() {
    for flag in [None, Some(false)] {
        let scene: Option<Scene> = group_scene(flag, || panic!("non-group source read"));
        enum_group_items(scene, |_| panic!("non-group enumerated"));
    }
}

#[test]
fn absent_scene() {
    enum_group_items(group_scene(Some(true), || None::<Scene>), |_| {
        panic!("absent scene enumerated")
    });
}

#[test]
fn empty_group_and_real_non_group() {
    let empty = Scene { id: 7, items: &[] };
    enum_group_items(group_scene(Some(false), || Some(empty)), |_| {
        panic!("plain scene item enumerated")
    });
    let mut calls = 0;
    enum_group_items(Some(empty), |scene| {
        calls += 1;
        assert_eq!(scene, empty);
        assert!(scene.items.is_empty());
    });
    assert_eq!(calls, 1, "even an empty scene is delegated");
}

fn check(stop_at: usize, param: Option<&i32>) {
    let group = Scene { id: 7, items: &[11, 23, 47] };
    let parent = Scene { id: 8, items: &[101, 7, 103] };
    let mut calls = 0;
    let mut visited = Vec::new();
    // A NULL callback remains the scene enumerator's responsibility.
    enum_group_items(Some(group), |scene| {
        calls += 1;
        assert_eq!(scene, group);
    });
    assert!(visited.is_empty());
    enum_group_items(Some(group), |scene| {
        calls += 1;
        assert_ne!(scene, parent);
        for &item in scene.items {
            assert_eq!(scene.id, group.id);
            assert_eq!(param.copied(), param.map(|_| 42));
            visited.push(item);
            if visited.len() == stop_at { break; }
        }
    });
    assert_eq!(visited, group.items[..if stop_at == 0 { 3 } else { stop_at }]);
    assert_eq!(param.copied(), param.map(|_| 42));
    visited.clear();
    enum_group_items(Some(group), |scene| {
        calls += 1;
        visited.extend_from_slice(scene.items);
    });
    assert_eq!(visited, group.items);
    assert_eq!(calls, 3);
}

#[test]
fn order_and_param() {
    let token = 42;
    check(0, Some(&token));
    check(0, None);
    assert_eq!(token, 42);
}

#[test]
fn early_stop() {
    let token = 42;
    check(1, Some(&token));
    check(2, Some(&token));
}
