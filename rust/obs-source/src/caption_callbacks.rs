//! Ordered caption subscriptions; duplicates and null tokens are retained.
use core::ffi::CStr;

pub use crate::callback_registry::Registry;

pub fn update<R: Registry>(
    registry: Option<&mut R>,
    pair: R::Pair,
    add: bool,
    log: impl FnOnce(&'static CStr),
) {
    crate::callback_registry::update(
        registry,
        pair,
        add,
        [
            c"obs_source_add_caption_callback",
            c"obs_source_remove_caption_callback",
        ],
        log,
    );
}
