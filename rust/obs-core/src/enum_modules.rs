//! Live module enumeration, using the existing opaque module-list access.
use crate::module_lists::Lists;

/// Visits the loaded list in order, reading each successor after the callback.
/// No snapshot, lock, lifetime pin or early-stop return is added. Callbacks may
/// enumerate again or prepend modules while the current nodes remain alive.
pub fn enumerate<L: Lists>(lists: &mut L, mut callback: impl FnMut(&mut L, L::Module)) {
    todo!("obs_enum_modules Rust swap")
}
