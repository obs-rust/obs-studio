//! Legacy main-canvas wrapper; the existing canvas getter acquires ownership.
/// Acquire the current main canvas exactly once and preserve the returned handle.
#[allow(unused_variables)]
pub fn get_main_canvas<C>(canvas: C, get_ref: impl FnOnce(C) -> C) -> C {
    todo!()
}
