//! Opaque handles cross the ABI; private callback layouts stay in C.
use crate::stagesurface_getters::{Host, Property, metadata};
use std::ffi::{c_char, c_void};
type Getter = unsafe extern "C" fn(*const c_void) -> u32;
unsafe extern "C" {
    fn obs_stagesurface_context() -> *mut c_void;
    fn obs_stagesurface_debug(function: *const c_char, reason: *const c_char);
    fn obs_stagesurface_width_field(graphics: *mut c_void) -> Getter;
    fn obs_stagesurface_height_field(graphics: *mut c_void) -> Getter;
    fn obs_stagesurface_format_field(graphics: *mut c_void) -> Getter;
}
struct Adapter(*mut c_void);
impl Host<*const c_void> for Adapter {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        let function = match function {
            "gs_stagesurface_get_width" => c"gs_stagesurface_get_width",
            "gs_stagesurface_get_height" => c"gs_stagesurface_get_height",
            "gs_stagesurface_get_color_format" => c"gs_stagesurface_get_color_format",
            _ => unreachable!("static getter name"),
        };
        let reason = match reason {
            "called while not in a graphics context" => c"called while not in a graphics context",
            "Null 'stagesurf' parameter" => c"Null 'stagesurf' parameter",
            _ => unreachable!("static diagnostic"),
        };
        // SAFETY: both strings live through this synchronous logging call.
        unsafe { obs_stagesurface_debug(function.as_ptr(), reason.as_ptr()) };
    }
    fn dispatch(&mut self, property: Property, surface: *const c_void) -> u32 {
        // SAFETY: the safe core checked context and surface before dispatch.
        // The owning thread retains its context and backend callbacks throughout.
        unsafe {
            let callback = match property {
                Property::Width => obs_stagesurface_width_field(self.0),
                Property::Height => obs_stagesurface_height_field(self.0),
                Property::ColorFormat => obs_stagesurface_format_field(self.0),
            };
            callback(surface)
        }
    }
}
unsafe fn get(surface: *const c_void, property: Property) -> u32 {
    // SAFETY: reads the current thread's context without retaining a reference.
    let mut host = Adapter(unsafe { obs_stagesurface_context() });
    metadata(&mut host, (!surface.is_null()).then_some(surface), property)
}
/// Get the backend stage-surface width, or zero for an invalid call.
/// # Safety
/// A non-null surface belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_stagesurface_get_width(surface: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(surface, Property::Width) }
}
/// Get the backend stage-surface height, or zero for an invalid call.
/// # Safety
/// A non-null surface belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_stagesurface_get_height(surface: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(surface, Property::Height) }
}
/// Get the backend color-format value, or GS_UNKNOWN for an invalid call.
/// # Safety
/// A non-null surface belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_stagesurface_get_color_format(surface: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(surface, Property::ColorFormat) }
}
