//! Source callback dispatch with a fresh owned empty-list fallback.
use core::ffi::CStr;

pub struct Source<D, F> {
    pub data: Option<D>,
    pub callback: Option<F>,
}

pub fn get<D, R, F: FnOnce(D) -> R>(
    source: Option<Source<D, F>>,
    create: impl FnOnce() -> R,
    log: impl FnOnce(&'static CStr),
) -> R {
    todo!()
}
