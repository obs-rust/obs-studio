//! Ordered audio-capture subscriptions; duplicates and null tokens are retained.
use core::ffi::CStr;

pub use crate::callback_registry::Registry;

pub fn update<R: Registry>(
    registry: Option<&mut R>,
    pair: R::Pair,
    add: bool,
    log: impl FnOnce(&'static CStr),
) {
    // RED stub: the registration is not ported yet.
    let _ = (registry, pair, add, log);
}
