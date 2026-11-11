//! Enumerate a borrowed group scene through the existing scene enumerator.

/// Delegate exactly once when the group's scene is present.
/// The enumerator owns callback ordering, early stop and snapshot semantics.
pub fn enum_group_items<T>(_scene: Option<T>, _enumerate_scene: impl FnOnce(T)) {
    todo!("port group enumeration delegation")
}
