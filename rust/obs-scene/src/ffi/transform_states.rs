//! Public scene/data API bridge for transform snapshots; no private layout access.
use crate::transform_states::{self as engine, Crop, Host, Info, Pass};
use std::ffi::{c_char, c_void};
use std::ptr;
type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TransformInfo {
    pub pos: [f32; 2],
    pub rot: f32,
    pub scale: [f32; 2],
    pub alignment: u32,
    pub bounds_type: u32,
    pub bounds_alignment: u32,
    pub bounds: [f32; 2],
    pub crop_to_bounds: bool,
}
impl From<TransformInfo> for Info {
    fn from(v: TransformInfo) -> Self {
        Self {
            pos: v.pos,
            rot: v.rot,
            scale: v.scale,
            alignment: v.alignment,
            bounds_type: v.bounds_type,
            bounds_alignment: v.bounds_alignment,
            bounds: v.bounds,
            crop_to_bounds: v.crop_to_bounds,
        }
    }
}
impl From<Info> for TransformInfo {
    fn from(v: Info) -> Self {
        Self {
            pos: v.pos,
            rot: v.rot,
            scale: v.scale,
            alignment: v.alignment,
            bounds_type: v.bounds_type,
            bounds_alignment: v.bounds_alignment,
            bounds: v.bounds,
            crop_to_bounds: v.crop_to_bounds,
        }
    }
}
unsafe extern "C" {
    fn obs_data_create() -> Handle;
    fn obs_data_array_create() -> Handle;
    fn obs_data_create_from_json(json: *const c_char) -> Handle;
    fn obs_data_release(h: Handle);
    fn obs_data_array_release(h: Handle);
    fn obs_source_release(h: Handle);
    fn obs_data_get_string(h: Handle, key: *const c_char) -> *const c_char;
    fn obs_data_set_string(h: Handle, key: *const c_char, v: *const c_char);
    fn obs_data_get_int(h: Handle, key: *const c_char) -> i64;
    fn obs_data_set_int(h: Handle, key: *const c_char, v: i64);
    fn obs_data_get_double(h: Handle, key: *const c_char) -> f64;
    fn obs_data_set_double(h: Handle, key: *const c_char, v: f64);
    fn obs_data_get_bool(h: Handle, key: *const c_char) -> bool;
    fn obs_data_set_bool(h: Handle, key: *const c_char, v: bool);
    fn obs_data_get_vec2(h: Handle, key: *const c_char, v: *mut [f32; 2]);
    fn obs_data_set_vec2(h: Handle, key: *const c_char, v: *const [f32; 2]);
    fn obs_data_get_array(h: Handle, key: *const c_char) -> Handle;
    fn obs_data_set_array(h: Handle, key: *const c_char, a: Handle);
    fn obs_data_array_push_back(a: Handle, d: Handle) -> usize;
    fn obs_scene_get_source(h: Handle) -> Handle;
    fn obs_sceneitem_get_source(h: Handle) -> Handle;
    fn obs_source_is_group(h: Handle) -> bool;
    fn obs_source_get_name(h: Handle) -> *const c_char;
    fn obs_source_get_uuid(h: Handle) -> *const c_char;
    fn obs_sceneitem_selected(h: Handle) -> bool;
    fn obs_sceneitem_get_id(h: Handle) -> i64;
    fn obs_get_source_by_uuid(h: *const c_char) -> Handle;
    fn obs_scene_from_source(h: Handle) -> Handle;
    fn obs_sceneitem_group_get_scene(h: Handle) -> Handle;
    fn obs_scene_find_sceneitem_by_id(h: Handle, id: i64) -> Handle;
    fn obs_scene_get_group(h: Handle, name: *const c_char) -> Handle;
    fn obs_sceneitem_defer_update_begin(h: Handle);
    fn obs_sceneitem_defer_update_end(h: Handle);
    fn obs_sceneitem_get_info2(h: Handle, v: *mut TransformInfo);
    fn obs_sceneitem_get_crop(h: Handle, v: *mut Crop);
    fn obs_sceneitem_set_info2(h: Handle, v: *const TransformInfo);
    fn obs_sceneitem_set_crop(h: Handle, v: *const Crop);
    fn obs_scene_enum_items(
        h: Handle,
        callback: unsafe extern "C" fn(Handle, Handle, Handle) -> bool,
        data: Handle,
    );
    fn obs_sceneitem_group_enum_items(
        h: Handle,
        callback: unsafe extern "C" fn(Handle, Handle, Handle) -> bool,
        data: Handle,
    );
    fn obs_data_array_enum(h: Handle, callback: unsafe extern "C" fn(Handle, Handle), data: Handle);
}
fn key(k: &'static str) -> *const c_char {
    match k {
        "id" => c"id".as_ptr(),
        "pos" => c"pos".as_ptr(),
        "scale" => c"scale".as_ptr(),
        "rot" => c"rot".as_ptr(),
        "alignment" => c"alignment".as_ptr(),
        "bounds_type" => c"bounds_type".as_ptr(),
        "bounds" => c"bounds".as_ptr(),
        "bounds_alignment" => c"bounds_alignment".as_ptr(),
        "crop_to_bounds" => c"crop_to_bounds".as_ptr(),
        "top" => c"top".as_ptr(),
        "bottom" => c"bottom".as_ptr(),
        "left" => c"left".as_ptr(),
        "right" => c"right".as_ptr(),
        "scene_name" => c"scene_name".as_ptr(),
        "scene_uuid" => c"scene_uuid".as_ptr(),
        "is_group" => c"is_group".as_ptr(),
        "group_parent" => c"group_parent".as_ptr(),
        "items" => c"items".as_ptr(),
        "scenes_and_groups" => c"scenes_and_groups".as_ptr(),
        _ => unreachable!("transform snapshot key"),
    }
}
struct C;
impl Host for C {
    type Handle = Handle;
    type Text = *const c_char;
    fn create_data(&mut self) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_create() }
    }
    fn create_array(&mut self) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_array_create() }
    }
    fn parse(&mut self, json: Self::Text) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_create_from_json(json) }
    }
    fn release_data(&mut self, h: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_release(h) }
    }
    fn release_array(&mut self, h: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_array_release(h) }
    }
    fn release_source(&mut self, h: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_source_release(h) }
    }
    fn text(&mut self, h: Handle, k: &'static str) -> Self::Text {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_get_string(h, key(k)) }
    }
    fn set_text(&mut self, h: Handle, k: &'static str, v: Self::Text) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_set_string(h, key(k), v) }
    }
    fn int(&mut self, h: Handle, k: &'static str) -> i64 {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_get_int(h, key(k)) }
    }
    fn set_int(&mut self, h: Handle, k: &'static str, v: i64) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_set_int(h, key(k), v) }
    }
    fn double(&mut self, h: Handle, k: &'static str) -> f64 {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_get_double(h, key(k)) }
    }
    fn set_double(&mut self, h: Handle, k: &'static str, v: f64) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_set_double(h, key(k), v) }
    }
    fn boolean(&mut self, h: Handle, k: &'static str) -> bool {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_get_bool(h, key(k)) }
    }
    fn set_bool(&mut self, h: Handle, k: &'static str, v: bool) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_set_bool(h, key(k), v) }
    }
    fn vec2(&mut self, h: Handle, k: &'static str) -> [f32; 2] {
        let mut v = [0.; 2];
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe {
            obs_data_get_vec2(h, key(k), &mut v);
            v
        }
    }
    fn set_vec2(&mut self, h: Handle, k: &'static str, v: [f32; 2]) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_set_vec2(h, key(k), &v) }
    }
    fn array(&mut self, h: Handle, k: &'static str) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_get_array(h, key(k)) }
    }
    fn set_array(&mut self, h: Handle, k: &'static str, a: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_data_set_array(h, key(k), a) }
    }
    fn push(&mut self, a: Handle, d: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe {
            obs_data_array_push_back(a, d);
        }
    }
    fn source(&mut self, h: Handle) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_scene_get_source(h) }
    }
    fn item_source(&mut self, h: Handle) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_get_source(h) }
    }
    fn is_group(&mut self, h: Handle) -> bool {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_source_is_group(h) }
    }
    fn name(&mut self, h: Handle) -> Self::Text {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_source_get_name(h) }
    }
    fn uuid(&mut self, h: Handle) -> Self::Text {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_source_get_uuid(h) }
    }
    fn selected(&mut self, h: Handle) -> bool {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_selected(h) }
    }
    fn id(&mut self, h: Handle) -> i64 {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_get_id(h) }
    }
    fn lookup(&mut self, h: Self::Text) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_get_source_by_uuid(h) }
    }
    fn scene(&mut self, h: Handle) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_scene_from_source(h) }
    }
    fn group_scene(&mut self, h: Handle) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_group_get_scene(h) }
    }
    fn find(&mut self, h: Handle, id: i64) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_scene_find_sceneitem_by_id(h, id) }
    }
    fn group(&mut self, h: Handle, name: Self::Text) -> Handle {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_scene_get_group(h, name) }
    }
    fn defer_begin(&mut self, h: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_defer_update_begin(h) }
    }
    fn defer_end(&mut self, h: Handle) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_defer_update_end(h) }
    }
    fn info(&mut self, h: Handle) -> Info {
        let mut v = TransformInfo::default();
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe {
            obs_sceneitem_get_info2(h, &mut v);
            v.into()
        }
    }
    fn crop(&mut self, h: Handle) -> Crop {
        let mut v = Crop::default();
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe {
            obs_sceneitem_get_crop(h, &mut v);
            v
        }
    }
    fn set_info(&mut self, h: Handle, v: Info) {
        let v = TransformInfo::from(v);
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_set_info2(h, &v) }
    }
    fn set_crop(&mut self, h: Handle, v: Crop) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_set_crop(h, &v) }
    }
    fn enum_scene(&mut self, h: Handle, pass: &mut Pass<Handle>) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_scene_enum_items(h, save_callback, ptr::from_mut(pass).cast()) }
    }
    fn enum_group(&mut self, h: Handle, pass: &mut Pass<Handle>) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe { obs_sceneitem_group_enum_items(h, save_callback, ptr::from_mut(pass).cast()) }
    }
    fn enum_data(&mut self, a: Handle, scene: Handle, groups: bool) {
        // SAFETY: C is constructed only under the exported live-handle contract; public APIs accept null handles.
        unsafe {
            obs_data_array_enum(
                a,
                if groups {
                    group_callback
                } else {
                    load_callback
                },
                scene,
            )
        }
    }
    fn null(&self) -> Handle {
        ptr::null_mut()
    }
}
unsafe extern "C" fn save_callback(scene: Handle, item: Handle, pass: Handle) -> bool {
    // SAFETY: enum_scene/enum_group pass a live, uniquely borrowed Pass for synchronous enumeration.
    engine::save_item(&mut C, scene, item, unsafe {
        &mut *pass.cast::<Pass<Handle>>()
    });
    true
}
unsafe extern "C" fn load_callback(data: Handle, scene: Handle) {
    engine::load_item(&mut C, data, scene);
}
unsafe extern "C" fn group_callback(data: Handle, _unused: Handle) {
    engine::load_record(&mut C, data);
}
/// # Safety
/// Scene must be null or live and synchronized according to the public scene API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_save_transform_states(scene: Handle, all_items: bool) -> Handle {
    engine::save(&mut C, scene, all_items)
}
/// # Safety
/// Data must point to a readable NUL-terminated JSON string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_load_transform_states(data: *const c_char) {
    engine::load(&mut C, data);
}
