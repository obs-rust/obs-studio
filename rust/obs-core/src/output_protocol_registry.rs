//! Read-only queries over the core's ordered protocol registry.

/// Match a complete protocol name, preserving case and arbitrary bytes.
pub fn registered<'a>(
    protocol: Option<&[u8]>,
    protocols: impl IntoIterator<Item = &'a [u8]>,
) -> bool {
    protocol.is_some_and(|protocol| crate::module_type_rules::protocol_listed(protocol, protocols))
}

/// Borrow the entry at `index`, without changing the registry.
pub fn enumerate<T: Copy>(protocols: &[T], index: usize) -> Option<T> {
    protocols.get(index).copied()
}
