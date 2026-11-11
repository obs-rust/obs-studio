//! Enumerate a borrowed group scene through the existing scene enumerator.

/// Delegate exactly once when the group's scene is present.
/// The enumerator owns callback ordering, early stop and snapshot semantics.
pub fn enum_group_items<T>(scene: Option<T>, enumerate_scene: impl FnOnce(T)) {
    if let Some(scene) = scene {
        enumerate_scene(scene);
    }
}
