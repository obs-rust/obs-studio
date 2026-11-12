//! Identifier-only source classification, independent of context and source type.

/// Test the registered scene identifier. Bytes exclude the C terminator.
pub fn is_scene(id: Option<&[u8]>) -> bool {
    crate::from_source::from_source(id, b"scene", || true)
}

/// Test the registered group identifier. Bytes exclude the C terminator.
pub fn is_group(id: Option<&[u8]>) -> bool {
    crate::from_source::from_source(id, b"group", || true)
}
