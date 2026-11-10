//! Opaque handles cross the ABI; backend callback layouts remain in C.
use crate::cubetexture_access::{Host, Operation, access};
use std::ffi::{c_char, c_void};
type Getter = unsafe extern "C" fn(*const c_void) -> u32;
type Destroy = unsafe extern "C" fn(*mut c_void);
unsafe extern "C" {
    fn obs_cubetexture_access_context() -> *mut c_void;
    fn obs_cubetexture_access_debug(function: *const c_char, reason: *const c_char);
    fn obs_cubetexture_access_size_field(graphics: *mut c_void) -> Getter;
    fn obs_cubetexture_access_destroy_field(graphics: *mut c_void) -> Destroy;
    fn obs_cubetexture_access_format_field(graphics: *mut c_void) -> Getter;
}
struct Adapter(*mut c_void);
impl Host<*const c_void> for Adapter {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        let function = match function {
            "gs_cubetexture_get_size" => c"gs_cubetexture_get_size",
            "gs_cubetexture_destroy" => c"gs_cubetexture_destroy",
            "gs_cubetexture_get_color_format" => c"gs_cubetexture_get_color_format",
            _ => unreachable!("static operation name"),
        };
        let reason = match reason {
            "called while not in a graphics context" => c"called while not in a graphics context",
            "Null 'cubetex' parameter" => c"Null 'cubetex' parameter",
            _ => unreachable!("static diagnostic"),
        };
        // SAFETY: both strings live through this synchronous logging call.
        unsafe { obs_cubetexture_access_debug(function.as_ptr(), reason.as_ptr()) };
    }
    fn dispatch(&mut self, operation: Operation, texture: *const c_void) -> u32 {
        // SAFETY: the safe core checked context and texture. The owning thread
        // retains its context; only the backend owns texture destruction.
        unsafe {
            match operation {
                Operation::Size => obs_cubetexture_access_size_field(self.0)(texture),
                Operation::ColorFormat => obs_cubetexture_access_format_field(self.0)(texture),
                Operation::Destroy => {
                    obs_cubetexture_access_destroy_field(self.0)(texture.cast_mut());
                    0
                }
            }
        }
    }
}
fn call(texture: *const c_void, operation: Operation) -> u32 {
    // SAFETY: reads the current thread's context without retaining a reference.
    let mut host = Adapter(unsafe { obs_cubetexture_access_context() });
    access(
        &mut host,
        (!texture.is_null()).then_some(texture),
        operation,
    )
}
/// Return the backend cube size, or zero for invalid calls.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_cubetexture_get_size(texture: *const c_void) -> u32 {
    call(texture, Operation::Size)
}
/// Return the backend color format, or GS_UNKNOWN for invalid calls.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_cubetexture_get_color_format(texture: *const c_void) -> u32 {
    call(texture, Operation::ColorFormat)
}
/// Release a cube texture through its owning backend.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context and must
/// not be accessed after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_cubetexture_destroy(texture: *mut c_void) {
    call(texture, Operation::Destroy);
}
