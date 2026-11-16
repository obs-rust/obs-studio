//! Main-canvas enumeration wrapper; traversal remains owned by the canvas API.
/// Forward the borrowed current canvas, callback and opaque parameter unchanged.
pub fn enum_scenes<C, F, P>(canvas: C, callback: F, param: P, enumerate: impl FnOnce(C, F, P)) {
    enumerate(canvas, callback, param);
}
