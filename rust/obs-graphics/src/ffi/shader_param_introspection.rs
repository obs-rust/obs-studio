//! Opaque handles and callback fields at the libobs ABI boundary.
use super::effect_passes::gs_shader_param_info;
use crate::shader_param_introspection::{self as core, Host};
use std::ffi::{CString, c_char, c_void};

type Count = unsafe extern "C" fn(*const c_void) -> i32;
type ByIndex = unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void;
type ByName = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type Info = unsafe extern "C" fn(*const c_void, *mut gs_shader_param_info);
unsafe extern "C" {
    fn obs_shader_param_introspection_context() -> *mut c_void;
    fn obs_shader_param_introspection_debug(function: *const c_char, reason: *const c_char);
    fn obs_shader_param_introspection_count_field(graphics: *mut c_void) -> Count;
    fn obs_shader_param_introspection_index_field(graphics: *mut c_void) -> ByIndex;
    fn obs_shader_param_introspection_name_field(graphics: *mut c_void) -> ByName;
    fn obs_shader_param_introspection_info_field(graphics: *mut c_void) -> Info;
}

struct Adapter(*mut c_void);
impl Adapter {
    fn current() -> Self {
        // SAFETY: reads only the current thread's graphics context.
        Self(unsafe { obs_shader_param_introspection_context() })
    }
}
impl Host for Adapter {
    type Handle = *const c_void;
    type Name = *const c_char;
    type Info = *mut gs_shader_param_info;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        let function = CString::new(function).expect("static function name");
        let reason = CString::new(reason).expect("static diagnostic");
        // SAFETY: both strings live through the synchronous C log call.
        unsafe { obs_shader_param_introspection_debug(function.as_ptr(), reason.as_ptr()) };
    }
    fn count(&mut self, shader: Self::Handle) -> i32 {
        // SAFETY: the core validated context and shader before accessing the callback.
        unsafe { obs_shader_param_introspection_count_field(self.0)(shader) }
    }
    fn by_index(&mut self, shader: Self::Handle, index: u32) -> Option<Self::Handle> {
        // SAFETY: validated opaque shader belongs to this thread's live context.
        let result =
            unsafe { obs_shader_param_introspection_index_field(self.0)(shader.cast_mut(), index) };
        (!result.is_null()).then_some(result.cast_const())
    }
    fn by_name(&mut self, shader: Self::Handle, name: Self::Name) -> Option<Self::Handle> {
        // SAFETY: the core validated both pointers and the context. No string is copied.
        let result =
            unsafe { obs_shader_param_introspection_name_field(self.0)(shader.cast_mut(), name) };
        (!result.is_null()).then_some(result.cast_const())
    }
    fn info(&mut self, param: Self::Handle, info: Self::Info) {
        // SAFETY: validated pointers are passed through without reading or initializing info.
        unsafe { obs_shader_param_introspection_info_field(self.0)(param, info) };
    }
}

/// Return the backend parameter count or zero when validation fails.
/// # Safety
/// A non-null shader belongs to the current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_get_num_params(shader: *const c_void) -> i32 {
    core::num_params(
        &mut Adapter::current(),
        (!shader.is_null()).then_some(shader),
    )
}
/// Return the backend's borrowed parameter handle for an index.
/// # Safety
/// A non-null shader belongs to the current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_get_param_by_idx(
    shader: *mut c_void,
    index: u32,
) -> *mut c_void {
    core::param_by_idx(
        &mut Adapter::current(),
        (!shader.is_null()).then_some(shader.cast_const()),
        index,
    )
    .unwrap_or(std::ptr::null())
    .cast_mut()
}
/// Return the backend's borrowed parameter handle for a name.
/// # Safety
/// Non-null shader and name pointers are live for the synchronous backend call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_get_param_by_name(
    shader: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    core::param_by_name(
        &mut Adapter::current(),
        (!shader.is_null()).then_some(shader.cast_const()),
        (!name.is_null()).then_some(name),
    )
    .unwrap_or(std::ptr::null())
    .cast_mut()
}
/// Let the backend write parameter information directly into the caller's output.
/// # Safety
/// Non-null param and writable info pointers are valid for the backend call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_get_param_info(
    param: *const c_void,
    info: *mut gs_shader_param_info,
) {
    core::param_info(
        &mut Adapter::current(),
        (!param.is_null()).then_some(param),
        (!info.is_null()).then_some(info),
    );
}
