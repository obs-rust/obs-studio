//! The source type lookup of `libobs/obs-source-info-lookup.c.h`,
//! `get_source_info()`: the first registered source type whose ID equals the
//! asked ID, bytewise like `strcmp`.
//!
//! The registry is read through [`Registry`], so the core never sees
//! `struct obs_source_info`: the C shim answers with the core's
//! `source_types` array and returns the C entry's own address.
//!
//! One intentional difference from the C: a NULL ID finds nothing. The C
//! passes it to `strcmp`, which crashes once any type is registered.

/// The core's list of registered source types, as the lookup reads it.
pub trait Registry {
    /// How many types are registered (`source_types.num`). Read again
    /// before each entry, as the C loop condition does.
    fn count(&mut self) -> usize;
    /// The ID of entry `index` (`source_types.array[index].id`), or `None`
    /// for a NULL ID, which no asked ID matches.
    fn id(&mut self, index: usize) -> Option<&[u8]>;
}

/// The index of the first entry whose ID is `id`, or `None`. A NULL `id`
/// (`None`) finds nothing and reads nothing. The asked ID is read (`id` is
/// called) only where the C compares it: once, at the first entry, so with
/// nothing registered it is never read.
pub fn find<'a, R: Registry + ?Sized>(
    registry: &mut R,
    id: Option<impl FnOnce() -> &'a [u8]>,
) -> Option<usize> {
    let id = id?();
    let mut index = 0;
    while index < registry.count() {
        if registry.id(index) == Some(id) {
            return Some(index);
        }
        index += 1;
    }
    None
}
