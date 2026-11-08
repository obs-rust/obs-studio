//! Exact byte queries for the built-in group and scene source identifiers.

/// Whether a nullable source identifier names a group.
pub fn is_group(id: Option<&[u8]>) -> bool {
    id == Some(b"group".as_slice())
}

/// Whether a nullable source identifier names a scene.
pub fn is_scene(id: Option<&[u8]>) -> bool {
    id == Some(b"scene".as_slice())
}
