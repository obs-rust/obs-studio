//! Ownership of the module loader's failure table. The count and borrowed
//! string entries are intentionally outside this cleanup's responsibility.

/// Release the table exactly once, retaining the C ordering (free, then clear).
pub fn free_table<T: Copy + Default + PartialEq>(table: &mut T, mut free: impl FnMut(T)) {
    if *table != T::default() {
        free(*table);
        *table = T::default();
    }
}
