//! Preserve the borrowed C pointer while keeping private core layout in C.
use crate::base_effect::{Field, get};
use std::ffi::c_void;

unsafe extern "C" {
    fn obs_core_initialized_field() -> *const *const c_void;
    fn obs_base_effect_default_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_default_rect_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_opaque_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_solid_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_bicubic_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_lanczos_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_bilinear_lowres_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_premultiplied_alpha_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_repeat_field(core: *const c_void) -> *mut c_void;
    fn obs_base_effect_area_field(core: *const c_void) -> *mut c_void;
}

/// Borrow one video-owned effect, or return NULL for an unknown enum value.
///
/// # Safety
/// A known enum value requires an initialized, live core. Video effects must
/// not be replaced or destroyed concurrently. The caller must not free the
/// returned pointer: ownership and lifetime remain with the core's video state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_base_effect(effect: i32) -> *mut c_void {
    get(effect, |field| {
        // SAFETY: the existing adapter returns the process-global core slot.
        let core = unsafe { *obs_core_initialized_field() };
        assert!(!core.is_null(), "known base effect requires a live core");
        // SAFETY: core is non-null and live by the caller contract. These real
        // private adapters copy pointer values without acquiring ownership.
        unsafe {
            match field {
                Field::Default => obs_base_effect_default_field(core),
                Field::DefaultRect => obs_base_effect_default_rect_field(core),
                Field::Opaque => obs_base_effect_opaque_field(core),
                Field::Solid => obs_base_effect_solid_field(core),
                Field::Bicubic => obs_base_effect_bicubic_field(core),
                Field::Lanczos => obs_base_effect_lanczos_field(core),
                Field::BilinearLowres => obs_base_effect_bilinear_lowres_field(core),
                Field::PremultipliedAlpha => obs_base_effect_premultiplied_alpha_field(core),
                Field::Repeat => obs_base_effect_repeat_field(core),
                Field::Area => obs_base_effect_area_field(core),
            }
        }
    })
}
