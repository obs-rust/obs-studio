//! Scalar conversion followed by the unchanged C vector submission boundary.
use super::vec3::vec3;

unsafe extern "C" {
    fn gs_vertex3v(value: *const vec3);
    fn gs_normal3v(value: *const vec3);
}

/// Submit one two-dimensional vertex through the existing vector API.
///
/// # Safety
/// The caller must obey the libobs immediate-mode graphics context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertex2f(x: f32, y: f32) {
    let value: vec3 = crate::immediate_scalars::vertex2f(x, y).into();
    // SAFETY: value has the public vec3 layout and lives through the synchronous
    // call, which copies it and retains all context and capacity validation.
    unsafe { gs_vertex3v(&value) };
}

/// Submit one normal through the existing vector API.
///
/// # Safety
/// The caller must obey the libobs immediate-mode graphics context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_normal3f(x: f32, y: f32, z: f32) {
    let value: vec3 = crate::immediate_scalars::normal3f(x, y, z).into();
    // SAFETY: value has the public vec3 layout and lives through the synchronous
    // call, which copies it and retains all context and capacity validation.
    unsafe { gs_normal3v(&value) };
}
