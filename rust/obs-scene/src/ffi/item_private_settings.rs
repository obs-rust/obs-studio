//! Read only `private_settings` of the real C item layout; the reference
//! goes through libobs' own `obs_data_addref`.
use super::{
    item_state_getters::SceneItemPrefix as StatePrefix, scene_bounds_place::SceneItemCrop,
};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;
use obs_graphics::ffi::{matrix4::matrix4, vec2::vec2};

/// Private C item prefix through `private_settings`. Only that field is read,
/// never the whole prefix: its final alignment padding may overlap the
/// opaque C suffix.
#[repr(C)]
pub struct SceneItemPrefix {
    pub state: StatePrefix,
    pub item_render: *mut c_void,
    pub crop: SceneItemCrop,
    pub absolute_coordinates: bool,
    pub pos: vec2,
    pub scale: vec2,
    pub scale_ref: vec2,
    pub rot: f32,
    pub align: u32,
    pub last_width: u32,
    pub last_height: u32,
    pub output_scale: vec2,
    pub scale_filter: u32,
    pub blend_method: u32,
    pub blend_type: u32,
    pub box_transform: matrix4,
    pub box_scale: vec2,
    pub draw_transform: matrix4,
    pub bounds_type: u32,
    pub bounds_align: u32,
    pub bounds: vec2,
    pub crop_to_bounds: bool,
    pub bounds_crop: SceneItemCrop,
    pub toggle_visibility: usize,
    pub private_settings: *mut c_void,
}

unsafe extern "C" {
    fn obs_data_addref(data: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Return the item's private settings with one added reference.
///
/// # Safety
/// A non-null item must be live and its `private_settings` readable, under
/// the same (unlocked) contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_private_settings(
    item: *const SceneItemPrefix,
) -> *mut c_void {
    let stored = if item.is_null() {
        None
    } else {
        // SAFETY: NULL is excluded; read only this field, as the C does.
        Some(unsafe { ptr::addr_of!((*item).private_settings).read() })
    };
    crate::item_private_settings::get(
        stored,
        |settings| {
            // SAFETY: the live item owns this reference; addref accepts NULL.
            unsafe { obs_data_addref(settings) }
        },
        |name: &'static CStr| {
            // SAFETY: the format and both variadic arguments are static C strings.
            unsafe {
                blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    name.as_ptr(),
                    c"item".as_ptr(),
                )
            }
        },
    )
}
