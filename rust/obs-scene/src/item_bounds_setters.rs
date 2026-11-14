//! The scene item bounds setters of `obs-scene-item-bounds-setters.c.h`:
//! the bounds type check, and the order each setter keeps (lock the item's
//! fields, store, unlock, then update the transform).

/// `OBS_BOUNDS_NONE`, the first `enum obs_bounds_type` value.
pub const BOUNDS_NONE: u32 = 0;
/// `OBS_BOUNDS_MAX_ONLY`, the last `enum obs_bounds_type` value.
pub const BOUNDS_MAX_ONLY: u32 = 6;

/// `valid_bounds_type`: a bounds type outside the enum stores
/// `OBS_BOUNDS_NONE`. It takes the `int64_t` a scene load reads, before any
/// cast truncates it.
#[must_use]
pub fn valid_bounds_type(value: i64) -> u32 {
    match u32::try_from(value) {
        Ok(kind) if kind <= BOUNDS_MAX_ONLY => kind,
        _ => BOUNDS_NONE,
    }
}

/// Why `obs_sceneitem_set_bounds` stores nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// No item: nothing happens, and nothing is logged.
    Item,
    /// No bounds: the item is checked first, then the pointer, which is
    /// logged and left alone before the lock is taken. The C read through it
    /// under the lock (an intentional difference).
    Bounds,
}

/// The null checks of `obs_sceneitem_set_bounds`, item first.
pub fn check(item: bool, bounds: bool) -> Result<(), Skip> {
    if !item {
        return Err(Skip::Item);
    }
    if !bounds {
        return Err(Skip::Bounds);
    }
    Ok(())
}

/// How `obs_sceneitem_set_bounds` stores the new size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// Relative coordinates: `size_from_absolute` converts it.
    Convert,
    /// Absolute coordinates: `vec2_copy` stores it as it is.
    Copy,
}

/// The store of `obs_sceneitem_set_bounds`, read under the lock from the
/// item's `absolute_coordinates`.
#[must_use]
pub fn store(absolute_coordinates: bool) -> Store {
    if absolute_coordinates {
        Store::Copy
    } else {
        Store::Convert
    }
}

/// The calls each setter makes around its store.
pub trait Fields {
    /// `item_fields_lock`: take the parent's video lock, if it has a parent.
    fn lock(&mut self);
    /// `item_fields_unlock`.
    fn unlock(&mut self);
    /// `do_update_transform`, after the unlock.
    fn update(&mut self);
}

/// One setter on a live item: lock, `write`, unlock, update.
pub fn set<F: Fields>(item: &mut F, write: impl FnOnce(&mut F)) {
    item.lock();
    write(item);
    item.unlock();
    item.update();
}
