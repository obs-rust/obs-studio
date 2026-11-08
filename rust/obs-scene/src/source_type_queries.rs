//! Exact byte queries for the built-in group and scene source identifiers.

/// Whether a nullable source identifier names a group.
pub fn is_group(_id: Option<&[u8]>) -> bool {
    todo!("port group source-type query")
}

/// Whether a nullable source identifier names a scene.
pub fn is_scene(_id: Option<&[u8]>) -> bool {
    todo!("port scene source-type query")
}
