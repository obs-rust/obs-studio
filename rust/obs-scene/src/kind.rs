//! Identifier-only source classification, independent of context and source type.

/// Test the registered scene identifier. Bytes exclude the C terminator.
pub fn is_scene(_id: Option<&[u8]>) -> bool {
    todo!("scene kind port")
}

/// Test the registered group identifier. Bytes exclude the C terminator.
pub fn is_group(_id: Option<&[u8]>) -> bool {
    todo!("group kind port")
}
