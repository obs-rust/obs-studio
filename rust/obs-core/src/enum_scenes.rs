//! Main-canvas enumeration wrapper; traversal remains owned by the canvas API.
/// Forward the borrowed current canvas, callback and opaque parameter unchanged.
pub fn enum_scenes<C, F, P>(_canvas: C, _callback: F, _param: P, _enumerate: impl FnOnce(C, F, P)) {
    todo!("RED: delegate scene enumeration");
}
