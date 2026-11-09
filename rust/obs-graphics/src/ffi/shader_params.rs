//! Opaque graphics and backend parameter boundary; no layout mirrors or dereferences.
use crate::shader_params::{self, WriteError};
use std::ffi::{c_char, c_void};
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_graphics_shader_params_context() -> *mut c_void;
    fn obs_graphics_shader_params_log(function: *const c_char, reason: *const c_char);
    fn obs_graphics_shader_params_bool(context: *mut c_void, param: *mut c_void, value: bool);
    fn obs_graphics_shader_params_float(context: *mut c_void, param: *mut c_void, value: f32);
    fn obs_graphics_shader_params_int(context: *mut c_void, param: *mut c_void, value: i32);
    fn obs_graphics_shader_params_matrix3(
        context: *mut c_void,
        param: *mut c_void,
        value: *const c_void,
    );
    fn obs_graphics_shader_params_matrix4(
        context: *mut c_void,
        param: *mut c_void,
        value: *const c_void,
    );
    fn obs_graphics_shader_params_vec2(
        context: *mut c_void,
        param: *mut c_void,
        value: *const c_void,
    );
    fn obs_graphics_shader_params_vec3(
        context: *mut c_void,
        param: *mut c_void,
        value: *const c_void,
    );
    fn obs_graphics_shader_params_vec4(
        context: *mut c_void,
        param: *mut c_void,
        value: *const c_void,
    );
}

fn report(result: Result<(), WriteError>, function: &std::ffi::CStr) {
    if let Err(error) = result {
        let reason = match error {
            WriteError::NoContext => c"called while not in a graphics context",
            WriteError::NullParameter => c"Null 'param' parameter",
            WriteError::NullValue => c"Null 'val' parameter",
        };
        // SAFETY: both strings remain live and NUL terminated through the log call.
        unsafe {
            obs_graphics_shader_params_log(function.as_ptr(), reason.as_ptr());
        }
    }
}

/// Forward a shader bool write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_bool(param: *mut c_void, value: bool) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        Some(value),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_bool(context, param.as_ptr(), value);
            }
        },
    );
    report(result, c"gs_shader_set_bool");
}

/// Forward a shader float write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_float(param: *mut c_void, value: f32) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        Some(value),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_float(context, param.as_ptr(), value);
            }
        },
    );
    report(result, c"gs_shader_set_float");
}

/// Forward a shader int write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_int(param: *mut c_void, value: i32) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        Some(value),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_int(context, param.as_ptr(), value);
            }
        },
    );
    report(result, c"gs_shader_set_int");
}

/// Forward a shader matrix3 write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_matrix3(param: *mut c_void, value: *const c_void) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        NonNull::new(value.cast_mut()),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_matrix3(context, param.as_ptr(), value.as_ptr());
            }
        },
    );
    report(result, c"gs_shader_set_matrix3");
}

/// Forward a shader matrix4 write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_matrix4(param: *mut c_void, value: *const c_void) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        NonNull::new(value.cast_mut()),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_matrix4(context, param.as_ptr(), value.as_ptr());
            }
        },
    );
    report(result, c"gs_shader_set_matrix4");
}

/// Forward a shader vec2 write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_vec2(param: *mut c_void, value: *const c_void) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        NonNull::new(value.cast_mut()),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_vec2(context, param.as_ptr(), value.as_ptr());
            }
        },
    );
    report(result, c"gs_shader_set_vec2");
}

/// Forward a shader vec3 write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_vec3(param: *mut c_void, value: *const c_void) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        NonNull::new(value.cast_mut()),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_vec3(context, param.as_ptr(), value.as_ptr());
            }
        },
    );
    report(result, c"gs_shader_set_vec3");
}

/// Forward a shader vec4 write to the current graphics backend.
///
/// # Safety
/// The calling thread must own its live graphics context and pass backend-valid
/// parameter and value tokens (null is accepted and diagnosed as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_set_vec4(param: *mut c_void, value: *const c_void) {
    // SAFETY: the adapter only reads the calling thread's context pointer.
    let context = unsafe { obs_graphics_shader_params_context() };
    let result = shader_params::write(
        !context.is_null(),
        NonNull::new(param),
        NonNull::new(value.cast_mut()),
        |param, value| {
            // SAFETY: the core validated all nulls; the caller guarantees backend
            // validity, and the adapter forwards without dereferencing these tokens.
            unsafe {
                obs_graphics_shader_params_vec4(context, param.as_ptr(), value.as_ptr());
            }
        },
    );
    report(result, c"gs_shader_set_vec4");
}
