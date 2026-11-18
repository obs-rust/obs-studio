//! Thin texture-copy ABI; private C adapters own graphics layouts.
use crate::texture_copy::{self as core, Entry, Host, Rejection};
use std::ffi::{c_char, c_void};
use std::ptr;

unsafe extern "C" {
    fn obs_graphics_texture_copy_context() -> *mut c_void;
    fn obs_graphics_texture_copy_debug(function: *const c_char, reason: *const c_char);
    fn obs_graphics_texture_copy_whole(context: *mut c_void, dst: *mut c_void, src: *mut c_void);
    fn obs_graphics_texture_copy_region(
        context: *mut c_void,
        dst: *mut c_void,
        dst_x: u32,
        dst_y: u32,
        src: *mut c_void,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
    );
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: reads the calling thread's context without dereferencing it.
    Context(unsafe { obs_graphics_texture_copy_context() })
}

fn optional(pointer: *mut c_void) -> Option<*mut c_void> {
    (!pointer.is_null()).then_some(pointer)
}

impl Host for Context {
    type Texture = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        // SAFETY: both strings are static and NUL-terminated.
        unsafe {
            obs_graphics_texture_copy_debug(entry.c_name().as_ptr(), reason.message().as_ptr())
        }
    }
    fn whole(&mut self, dst: *mut c_void, src: *mut c_void) {
        // SAFETY: Rust checked context, destination and source before field access.
        unsafe { obs_graphics_texture_copy_whole(self.0, dst, src) }
    }
    fn region(&mut self, dst: *mut c_void, src: Option<*mut c_void>, v: [u32; 6]) {
        // SAFETY: Rust checked context and destination before field access.
        // A NULL source and unchecked offsets/extents are the original contract.
        unsafe {
            obs_graphics_texture_copy_region(
                self.0,
                dst,
                v[0],
                v[1],
                src.unwrap_or(ptr::null_mut()),
                v[2],
                v[3],
                v[4],
                v[5],
            )
        }
    }
}

/// Copy a complete texture through the current backend.
/// # Safety
/// A current context must have a live backend; non-NULL textures must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_copy_texture(dst: *mut c_void, src: *mut c_void) {
    core::copy_texture(&mut current(), optional(dst), optional(src));
}

/// Copy a region, forwarding the source and all offsets/extents unchanged.
/// # Safety
/// A current context must have a live backend; non-NULL textures must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_copy_texture_region(
    dst: *mut c_void,
    dst_x: u32,
    dst_y: u32,
    src: *mut c_void,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) {
    core::copy_texture_region(
        &mut current(),
        optional(dst),
        optional(src),
        [dst_x, dst_y, src_x, src_y, src_w, src_h],
    );
}
