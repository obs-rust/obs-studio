//! Ordered selection from the complete source catalog, including non-input kinds.

/// Return the original borrowed ID of the first entry with the newest signed
/// version. A NULL query or no eligible match yields `None`; the ABI retains
/// the original debug assertion for a missing non-NULL query.
pub fn latest<'a, Id>(
    query: Option<&[u8]>,
    count: usize,
    entry: impl FnMut(usize) -> (&'a [u8], u32, Id),
) -> Option<Id> {
    let _ = (query, count, entry);
    todo!()
}
