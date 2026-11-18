//! Thin C ABI for device-load dispatch, over private C field adapters.
use crate::device_load::{self as load, Entry, Host};
use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_graphics_device_load_context() -> *mut c_void;
    fn obs_graphics_device_load_debug(function: *const c_char);
    fn obs_graphics_device_load_vertexbuffer(graphics: *mut c_void, buffer: *mut c_void);
    fn obs_graphics_device_load_indexbuffer(graphics: *mut c_void, buffer: *mut c_void);
    fn obs_graphics_device_load_texture(graphics: *mut c_void, texture: *mut c_void, unit: c_int);
    fn obs_graphics_device_load_samplerstate(
        graphics: *mut c_void,
        sampler: *mut c_void,
        unit: c_int,
    );
    fn obs_graphics_device_load_default_samplerstate(
        graphics: *mut c_void,
        b_3d: bool,
        unit: c_int,
    );
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads only this thread's current context; nothing is retained.
    Context(unsafe { obs_graphics_device_load_context() })
}

impl Host for Context {
    type Vertexbuffer = *mut c_void;
    type Indexbuffer = *mut c_void;
    type Texture = *mut c_void;
    type Samplerstate = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        // SAFETY: the name is a static NUL-terminated C string.
        unsafe { obs_graphics_device_load_debug(entry.c_name().as_ptr()) }
    }
    fn load_vertexbuffer(&mut self, buffer: *mut c_void) {
        // SAFETY: the core checked the context for NULL before field access.
        // The backend owns the device and handles, including NULL unbinds.
        unsafe { obs_graphics_device_load_vertexbuffer(self.0, buffer) }
    }
    fn load_indexbuffer(&mut self, buffer: *mut c_void) {
        // SAFETY: the core checked the context for NULL before field access.
        // The backend owns the device and handles, including NULL unbinds.
        unsafe { obs_graphics_device_load_indexbuffer(self.0, buffer) }
    }
    fn load_texture(&mut self, texture: *mut c_void, unit: i32) {
        // SAFETY: the core checked the context for NULL before field access.
        // The backend owns the device and handles, including NULL unbinds.
        unsafe { obs_graphics_device_load_texture(self.0, texture, unit) }
    }
    fn load_samplerstate(&mut self, sampler: *mut c_void, unit: i32) {
        // SAFETY: the core checked the context for NULL before field access.
        // The backend owns the device and handles, including NULL unbinds.
        unsafe { obs_graphics_device_load_samplerstate(self.0, sampler, unit) }
    }
    fn load_default_samplerstate(&mut self, b_3d: bool, unit: i32) {
        // SAFETY: the core checked the context for NULL before field access.
        unsafe { obs_graphics_device_load_default_samplerstate(self.0, b_3d, unit) }
    }
}

/// Forward a device load, including NULL unbinds.
/// # Safety
/// A current context must have a live backend device and vtable. Handles
/// must satisfy that backend's contract; this wrapper does not dereference them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_load_vertexbuffer(buffer: *mut c_void) {
    load::load_vertexbuffer(&mut current(), buffer);
}

/// Forward a device load, including NULL unbinds.
/// # Safety
/// A current context must have a live backend device and vtable. Handles
/// must satisfy that backend's contract; this wrapper does not dereference them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_load_indexbuffer(buffer: *mut c_void) {
    load::load_indexbuffer(&mut current(), buffer);
}

/// Forward a device load, including NULL unbinds.
/// # Safety
/// A current context must have a live backend device and vtable. Handles
/// must satisfy that backend's contract; this wrapper does not dereference them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_load_texture(texture: *mut c_void, unit: c_int) {
    load::load_texture(&mut current(), texture, unit);
}

/// Forward a device load, including NULL unbinds.
/// # Safety
/// A current context must have a live backend device and vtable. Handles
/// must satisfy that backend's contract; this wrapper does not dereference them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_load_samplerstate(sampler: *mut c_void, unit: c_int) {
    load::load_samplerstate(&mut current(), sampler, unit);
}

/// Forward the default sampler's 3D flag and unit unchanged.
/// # Safety
/// A current context must have a live backend device and vtable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_load_default_samplerstate(b_3d: bool, unit: c_int) {
    load::load_default_samplerstate(&mut current(), b_3d, unit);
}
