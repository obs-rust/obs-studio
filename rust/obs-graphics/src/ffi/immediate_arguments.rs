//! Public C argument adapters; validation stays in the vector submission API.
use core::ffi::c_int;

use super::{vec2::vec2, vec3::vec3};
use crate::immediate_arguments;

unsafe extern "C" {
    fn gs_vertex3v(value: *const vec3);
    fn gs_texcoord2v(value: *const vec2, unit: c_int);
}

/// Submit a three-dimensional vertex through the existing vector API.
///
/// # Safety
/// The caller must obey the libobs immediate-mode graphics context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertex3f(x: f32, y: f32, z: f32) {
    let value: vec3 = immediate_arguments::vertex3f(x, y, z).into();
    // SAFETY: the public-layout vector lives through the synchronous copying call.
    unsafe { gs_vertex3v(&value) };
}

/// Submit a texture coordinate through the existing vector API.
///
/// # Safety
/// The caller must obey the libobs graphics context and texture-unit contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texcoord(x: f32, y: f32, unit: c_int) {
    let (value, unit) = immediate_arguments::texcoord(x, y, unit);
    let value: vec2 = value.into();
    // SAFETY: the vector lives through the copying call; unit is forwarded unchanged.
    unsafe { gs_texcoord2v(&value, unit) };
}

/// Submit a two-dimensional vertex through the existing vector API.
///
/// # Safety
/// `value` must be aligned and readable, even without a current graphics context,
/// as in C. The caller must obey the libobs immediate-mode context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertex2v(value: *const vec2) {
    // SAFETY: the caller provides a live vec2. Read it before downstream validation.
    let input = unsafe { *value };
    let value: vec3 = immediate_arguments::vertex2v(input.into()).into();
    // SAFETY: the public-layout vector lives through the synchronous copying call.
    unsafe { gs_vertex3v(&value) };
}
