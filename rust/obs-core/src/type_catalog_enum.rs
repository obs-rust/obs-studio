//! Bounds and borrowed output for the six independent registered type catalogs.

pub fn enumerate<T>(
    count: usize,
    index: usize,
    read: impl FnOnce(usize) -> T,
    write: impl FnOnce(T),
) -> bool {
    let _ = (count, index, read, write);
    todo!("#5202: swap type catalog enumeration")
}
