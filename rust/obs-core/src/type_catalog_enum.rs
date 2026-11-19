//! Bounds and borrowed output for the six independent registered type catalogs.

pub fn enumerate<T>(
    count: usize,
    index: usize,
    read: impl FnOnce(usize) -> T,
    write: impl FnOnce(T),
) -> bool {
    crate::source_type_enum::source(count, index, read, write)
}
