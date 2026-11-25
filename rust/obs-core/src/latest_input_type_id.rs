//! Ordered selection from the complete source catalog, including non-input kinds.

/// Return the original borrowed ID of the first entry with the newest signed
/// version. A NULL query or no eligible match yields `None`; the ABI retains
/// the original debug assertion for a missing non-NULL query.
pub fn latest<'a, Id>(
    query: Option<&[u8]>,
    count: usize,
    mut entry: impl FnMut(usize) -> (&'a [u8], u32, Id),
) -> Option<Id> {
    let query = query?;
    let mut best = crate::core_setup_rules::LATEST_NONE;
    let mut selected = None;
    for i in 0..count {
        let (name, version, id) = entry(i);
        if name == query && crate::core_setup_rules::latest_newer(version, best) {
            best = version.cast_signed();
            selected = Some(id);
        }
    }
    selected
}
