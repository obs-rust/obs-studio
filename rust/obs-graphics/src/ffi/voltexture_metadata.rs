//! Opaque handles cross the ABI; private callback layouts stay in C.
use crate::voltexture_metadata::{Host, Property, metadata};
use std::ffi::{c_char, c_void};
type Getter = unsafe extern "C" fn(*const c_void) -> u32;
unsafe extern "C" {
    fn obs_voltexture_metadata_context() -> *mut c_void;
    fn obs_voltexture_metadata_debug(function: *const c_char, reason: *const c_char);
    fn obs_voltexture_metadata_width_field(graphics: *mut c_void) -> Getter;
    fn obs_voltexture_metadata_height_field(graphics: *mut c_void) -> Getter;
    fn obs_voltexture_metadata_depth_field(graphics: *mut c_void) -> Getter;
    fn obs_voltexture_metadata_format_field(graphics: *mut c_void) -> Getter;
}
struct Adapter(*mut c_void);
impl Host<*const c_void> for Adapter {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        let function = match function {
            "gs_voltexture_get_width" => c"gs_voltexture_get_width",
            "gs_voltexture_get_height" => c"gs_voltexture_get_height",
            "gs_voltexture_get_depth" => c"gs_voltexture_get_depth",
            "gs_voltexture_get_color_format" => c"gs_voltexture_get_color_format",
            _ => unreachable!("static getter name"),
        };
        let reason = match reason {
            "called while not in a graphics context" => c"called while not in a graphics context",
            "Null 'voltex' parameter" => c"Null 'voltex' parameter",
            _ => unreachable!("static diagnostic"),
        };
        // SAFETY: both strings live through this synchronous logging call.
        unsafe { obs_voltexture_metadata_debug(function.as_ptr(), reason.as_ptr()) };
    }
    fn dispatch(&mut self, property: Property, texture: *const c_void) -> u32 {
        // SAFETY: the safe core checked context and texture before dispatch.
        // The owning thread retains its context and backend callbacks throughout.
        unsafe {
            let callback = match property {
                Property::Width => obs_voltexture_metadata_width_field(self.0),
                Property::Height => obs_voltexture_metadata_height_field(self.0),
                Property::Depth => obs_voltexture_metadata_depth_field(self.0),
                Property::ColorFormat => obs_voltexture_metadata_format_field(self.0),
            };
            callback(texture)
        }
    }
}
unsafe fn get(texture: *const c_void, property: Property) -> u32 {
    // SAFETY: reads the current thread's context without retaining a reference.
    let mut host = Adapter(unsafe { obs_voltexture_metadata_context() });
    metadata(&mut host, (!texture.is_null()).then_some(texture), property)
}
/// Get the backend texture width, or zero for an invalid call.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_voltexture_get_width(texture: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(texture, Property::Width) }
}
/// Get the backend texture height, or zero for an invalid call.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_voltexture_get_height(texture: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(texture, Property::Height) }
}
/// Get the backend texture depth, or zero for an invalid call.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_voltexture_get_depth(texture: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(texture, Property::Depth) }
}
/// Get the backend color-format value, or GS_UNKNOWN for an invalid call.
/// # Safety
/// A non-null texture belongs to the current thread's graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_voltexture_get_color_format(texture: *const c_void) -> u32 {
    // SAFETY: caller supplies the graphics ownership contract.
    unsafe { get(texture, Property::ColorFormat) }
}
