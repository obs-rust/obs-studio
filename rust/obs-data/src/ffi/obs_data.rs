//! C ABI for `libobs/obs-data.h`.
//!
//! `obs_data_newref` stays inline in the header. Item pointers stay stable
//! across set (C may realloc the item). A null name, or a null vec
//! out-pointer, is a no-op. File loads and saves call `os_quick_*` so the
//! backup rename and the log lines match platform.c. The safe core's file
//! helpers are for the Rust tests.
//!
//! Each export is the C function of the same name. A null handle is a
//! no-op. A non-null handle is a pointer one of these functions returned
//! and that the caller has not released.
#![allow(clippy::missing_safety_doc)]

use std::borrow::Cow;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;
use std::sync::{Arc, RwLock};

use crate::data::{
    Array, ArrayInner, Data, DataInner, Item, ItemNode, Quat, Type, Vec2, Vec3, Vec4,
};

const LOG_WARNING: c_int = 200;

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default)]
pub struct vec2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C, align(16))]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default)]
pub struct vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C, align(16))]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default)]
pub struct vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C, align(16))]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default)]
pub struct quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default)]
pub struct media_frames_per_second {
    pub numerator: u32,
    pub denominator: u32,
}

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
    fn bfree(ptr: *mut c_void);
    fn os_file_exists(path: *const c_char) -> bool;
    fn os_rename(old_path: *const c_char, new_path: *const c_char) -> c_int;
    fn os_quick_read_utf8_file(path: *const c_char) -> *mut c_char;
    fn os_quick_write_utf8_file(
        path: *const c_char,
        str_: *const c_char,
        len: usize,
        marker: bool,
    ) -> bool;
    fn os_quick_write_utf8_file_safe(
        path: *const c_char,
        str_: *const c_char,
        len: usize,
        marker: bool,
        temp_ext: *const c_char,
        backup_ext: *const c_char,
    ) -> bool;
}

/// The text of a C string. Borrowed when it is valid UTF-8 (every real
/// key), so a get or set does not allocate for the name (#1453).
fn c_str<'a>(ptr: *const c_char) -> Option<Cow<'a, str>> {
    if ptr.is_null() {
        None
    } else {
        // SAFETY: `ptr` is a C string that outlives the call, or null
        // (handled above).
        Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy())
    }
}

fn borrow_data(ptr: *mut c_void) -> Option<Data> {
    if ptr.is_null() {
        return None;
    }
    let raw = ptr as *const RwLock<DataInner>;
    // SAFETY: `ptr` is a live data object. The extra count drops with the
    // returned value and leaves the caller's reference.
    unsafe { Arc::increment_strong_count(raw) };
    Some(Data {
        inner: unsafe { Arc::from_raw(raw) },
    })
}

fn take_data(ptr: *mut c_void) -> Option<Data> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: consumes the caller's reference.
    Some(Data {
        inner: unsafe { Arc::from_raw(ptr as *const RwLock<DataInner>) },
    })
}

fn leak_data(data: Data) -> *mut c_void {
    Arc::into_raw(data.inner) as *mut c_void
}

fn borrow_item(ptr: *mut c_void) -> Option<Item> {
    if ptr.is_null() {
        return None;
    }
    let raw = ptr as *const ItemNode;
    // SAFETY: same extra-count borrow as `borrow_data`.
    unsafe { Arc::increment_strong_count(raw) };
    Some(Item {
        inner: unsafe { Arc::from_raw(raw) },
    })
}

fn take_item(ptr: *mut c_void) -> Option<Item> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: consumes the caller's reference.
    Some(Item {
        inner: unsafe { Arc::from_raw(ptr as *const ItemNode) },
    })
}

fn leak_item(item: Item) -> *mut c_void {
    Arc::into_raw(item.inner) as *mut c_void
}

fn item_slot(item: *mut *mut c_void) -> Option<Item> {
    if item.is_null() {
        None
    } else {
        borrow_item(unsafe { *item })
    }
}

fn borrow_array(ptr: *mut c_void) -> Option<Array> {
    if ptr.is_null() {
        return None;
    }
    let raw = ptr as *const RwLock<ArrayInner>;
    // SAFETY: same extra-count borrow as `borrow_data`.
    unsafe { Arc::increment_strong_count(raw) };
    Some(Array {
        inner: unsafe { Arc::from_raw(raw) },
    })
}

fn take_array(ptr: *mut c_void) -> Option<Array> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: consumes the caller's reference.
    Some(Array {
        inner: unsafe { Arc::from_raw(ptr as *const RwLock<ArrayInner>) },
    })
}

fn leak_array(array: Array) -> *mut c_void {
    Arc::into_raw(array.inner) as *mut c_void
}

fn named<R>(data: *mut c_void, name: *const c_char, f: impl FnOnce(&Data, &str) -> R) -> Option<R> {
    let data = borrow_data(data)?;
    let name = c_str(name)?;
    Some(f(&data, &name))
}

fn empty() -> *const c_char {
    c"".as_ptr()
}

fn load_file(path: *const c_char) -> *mut c_void {
    if path.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `os_quick_read_utf8_file` returns a bmalloc buffer or null.
    let buf = unsafe { os_quick_read_utf8_file(path) };
    if buf.is_null() {
        return ptr::null_mut();
    }
    let text = unsafe { CStr::from_ptr(buf) }.to_string_lossy();
    let data = Data::from_json(&text);
    unsafe { bfree(buf.cast()) };
    data.map(leak_data).unwrap_or(ptr::null_mut())
}

fn write_json(data: *mut c_void, file: *const c_char, pretty: bool) -> Option<CString> {
    if file.is_null() {
        return None;
    }
    let data = borrow_data(data)?;
    let text = if pretty {
        data.json_pretty()
    } else {
        data.json()
    };
    if text.is_empty() {
        return None;
    }
    CString::new(text).ok()
}

fn frames_obj(fps: media_frames_per_second, option: *const c_char) -> Data {
    let obj = Data::new();
    if let Some(option) = c_str(option) {
        obj.set_string("option", Some(&option));
    } else {
        obj.set_double("numerator", f64::from(fps.numerator));
        obj.set_double("denominator", f64::from(fps.denominator));
    }
    obj
}

fn finish_fps(
    obj: &Data,
    read_option: bool,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    let read = obj.read_fps(read_option);
    if read.option.is_some() {
        if !option.is_null() {
            // SAFETY: the caller asked for the option string. It stays valid
            // while the parent object holds this frames object.
            unsafe { *option = obj.option_ptr() };
        }
        return true;
    }
    if fps.is_null() {
        return false;
    }
    if let Some(value) = read.fps {
        // SAFETY: `fps` is non-null. `w` is not a field here.
        unsafe {
            (*fps).numerator = value.numerator;
            (*fps).denominator = value.denominator;
        }
        return read.ok;
    }
    false
}

fn set_vec2(data: *mut c_void, name: *const c_char, val: *const vec2, slot: u8) {
    let Some((data, name)) = named_pair(data, name) else {
        return;
    };
    if val.is_null() {
        return;
    }
    let v = unsafe { *val };
    let value = Vec2 { x: v.x, y: v.y };
    match slot {
        1 => data.set_default_vec2(&name, value),
        2 => data.set_autoselect_vec2(&name, value),
        _ => data.set_vec2(&name, value),
    }
}

fn set_vec3(data: *mut c_void, name: *const c_char, val: *const vec3, slot: u8) {
    let Some((data, name)) = named_pair(data, name) else {
        return;
    };
    if val.is_null() {
        return;
    }
    let v = unsafe { *val };
    let value = Vec3 {
        x: v.x,
        y: v.y,
        z: v.z,
        w: v.w,
    };
    match slot {
        1 => data.set_default_vec3(&name, value),
        2 => data.set_autoselect_vec3(&name, value),
        _ => data.set_vec3(&name, value),
    }
}

fn set_vec4(data: *mut c_void, name: *const c_char, val: *const vec4, slot: u8) {
    let Some((data, name)) = named_pair(data, name) else {
        return;
    };
    if val.is_null() {
        return;
    }
    let v = unsafe { *val };
    let value = Vec4 {
        x: v.x,
        y: v.y,
        z: v.z,
        w: v.w,
    };
    match slot {
        1 => data.set_default_vec4(&name, value),
        2 => data.set_autoselect_vec4(&name, value),
        _ => data.set_vec4(&name, value),
    }
}

fn set_quat(data: *mut c_void, name: *const c_char, val: *const quat, slot: u8) {
    let Some((data, name)) = named_pair(data, name) else {
        return;
    };
    if val.is_null() {
        return;
    }
    let v = unsafe { *val };
    let value = Quat {
        x: v.x,
        y: v.y,
        z: v.z,
        w: v.w,
    };
    match slot {
        1 => data.set_default_quat(&name, value),
        2 => data.set_autoselect_quat(&name, value),
        _ => data.set_quat(&name, value),
    }
}

fn named_pair<'a>(data: *mut c_void, name: *const c_char) -> Option<(Data, Cow<'a, str>)> {
    Some((borrow_data(data)?, c_str(name)?))
}

fn get_vec2(data: *mut c_void, name: *const c_char, val: *mut vec2, slot: u8) {
    if val.is_null() {
        return;
    }
    let Some(got) = named(data, name, |data, name| match slot {
        1 => data.get_default_vec2(name),
        2 => data.get_autoselect_vec2(name),
        _ => data.get_vec2(name),
    })
    .flatten() else {
        return;
    };
    // SAFETY: `val` is non-null. A missing object leaves it unchanged.
    unsafe {
        (*val).x = got.x;
        (*val).y = got.y;
    }
}

fn get_vec3(data: *mut c_void, name: *const c_char, val: *mut vec3, slot: u8) {
    if val.is_null() {
        return;
    }
    let Some(got) = named(data, name, |data, name| match slot {
        1 => data.get_default_vec3(name),
        2 => data.get_autoselect_vec3(name),
        _ => data.get_vec3(name),
    })
    .flatten() else {
        return;
    };
    // SAFETY: `val` is non-null. `w` is left as the caller set it.
    unsafe {
        (*val).x = got.x;
        (*val).y = got.y;
        (*val).z = got.z;
    }
}

fn get_vec4(data: *mut c_void, name: *const c_char, val: *mut vec4, slot: u8) {
    if val.is_null() {
        return;
    }
    let Some(got) = named(data, name, |data, name| match slot {
        1 => data.get_default_vec4(name),
        2 => data.get_autoselect_vec4(name),
        _ => data.get_vec4(name),
    })
    .flatten() else {
        return;
    };
    unsafe {
        (*val).x = got.x;
        (*val).y = got.y;
        (*val).z = got.z;
        (*val).w = got.w;
    }
}

fn get_quat(data: *mut c_void, name: *const c_char, val: *mut quat, slot: u8) {
    if val.is_null() {
        return;
    }
    let Some(got) = named(data, name, |data, name| match slot {
        1 => data.get_default_quat(name),
        2 => data.get_autoselect_quat(name),
        _ => data.get_quat(name),
    })
    .flatten() else {
        return;
    };
    unsafe {
        (*val).x = got.x;
        (*val).y = got.y;
        (*val).z = got.z;
        (*val).w = got.w;
    }
}

fn set_frames(
    data: *mut c_void,
    name: *const c_char,
    fps: media_frames_per_second,
    option: *const c_char,
    slot: u8,
) {
    let Some((data, name)) = named_pair(data, name) else {
        return;
    };
    let obj = frames_obj(fps, option);
    match slot {
        1 => data.set_default_obj(&name, Some(&obj)),
        2 => data.set_autoselect_obj(&name, Some(&obj)),
        _ => data.set_obj(&name, Some(&obj)),
    }
}

fn get_frames(
    data: *mut c_void,
    name: *const c_char,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
    slot: u8,
) -> bool {
    let Some(obj) = named(data, name, |data, name| match slot {
        1 => data.get_default_obj(name),
        2 => data.get_autoselect_obj(name),
        _ => data.get_obj(name),
    })
    .flatten() else {
        return false;
    };
    finish_fps(&obj, !option.is_null(), fps, option)
}

fn set_item_frames(
    item: *mut *mut c_void,
    fps: media_frames_per_second,
    option: *const c_char,
    slot: u8,
) {
    let Some(item) = item_slot(item) else {
        return;
    };
    let obj = frames_obj(fps, option);
    match slot {
        1 => item.set_default_obj(Some(&obj)),
        2 => item.set_autoselect_obj(Some(&obj)),
        _ => item.set_obj(Some(&obj)),
    }
}

fn get_item_frames(
    item: *mut c_void,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
    slot: u8,
) -> bool {
    let Some(item) = borrow_item(item) else {
        return false;
    };
    let obj = match slot {
        1 => item.get_default_obj(),
        2 => item.get_autoselect_obj(),
        _ => item.get_obj(),
    };
    let Some(obj) = obj else {
        return false;
    };
    finish_fps(&obj, !option.is_null(), fps, option)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_create() -> *mut c_void {
    leak_data(Data::new())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_create_from_json(json_string: *const c_char) -> *mut c_void {
    let Some(text) = c_str(json_string) else {
        return ptr::null_mut();
    };
    Data::from_json(&text)
        .map(leak_data)
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_create_from_json_file(json_file: *const c_char) -> *mut c_void {
    load_file(json_file)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_create_from_json_file_safe(
    json_file: *const c_char,
    backup_ext: *const c_char,
) -> *mut c_void {
    let loaded = load_file(json_file);
    if !loaded.is_null() || backup_ext.is_null() || json_file.is_null() {
        return loaded;
    }
    // SAFETY: both pointers are non-null C strings. An empty extension skips
    // the backup, matching `backup_ext && *backup_ext`.
    if unsafe { *backup_ext } == 0 {
        return ptr::null_mut();
    }
    let path = unsafe { CStr::from_ptr(json_file) }.to_bytes();
    let ext = unsafe { CStr::from_ptr(backup_ext) }.to_bytes();
    let mut bytes = path.to_vec();
    if !ext.starts_with(b".") {
        bytes.push(b'.');
    }
    bytes.extend_from_slice(ext);
    let Ok(backup) = CString::new(bytes) else {
        return ptr::null_mut();
    };
    // SAFETY: `backup` is a C string. `os_rename` replaces the primary.
    unsafe {
        if !os_file_exists(backup.as_ptr()) {
            return ptr::null_mut();
        }
        blog(
            LOG_WARNING,
            c"obs-data.c: [obs_data_create_from_json_file_safe] attempting backup file".as_ptr(),
        );
        let _ = os_rename(backup.as_ptr(), json_file);
    }
    load_file(json_file)
}

/// The reference count of a live `obs_data_t`, for tests (#1451). Not a C
/// export.
///
/// # Safety
///
/// `data` is a live pointer from this module.
#[doc(hidden)]
pub unsafe fn ref_count_for_tests(data: *mut c_void) -> usize {
    let Some(data) = borrow_data(data) else {
        return 0;
    };
    // The borrow above holds one count of its own.
    Arc::strong_count(&data.inner) - 1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_addref(data: *mut c_void) {
    if !data.is_null() {
        // SAFETY: `data` is a live object, or we returned above.
        unsafe { Arc::increment_strong_count(data as *const RwLock<DataInner>) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_release(data: *mut c_void) {
    drop(take_data(data));
}

fn cached_json(data: *mut c_void, pretty: bool, defaults: bool) -> *const c_char {
    let Some(data) = borrow_data(data) else {
        return ptr::null();
    };
    // Rebuild the cached text straight into its CString: the C ABI returns
    // `json_ptr` below, so a `String` here would only be discarded (#1452).
    data.refresh_json(pretty, defaults);
    data.json_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_json(data: *mut c_void) -> *const c_char {
    cached_json(data, false, false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_json_with_defaults(data: *mut c_void) -> *const c_char {
    cached_json(data, false, true)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_json_pretty(data: *mut c_void) -> *const c_char {
    cached_json(data, true, false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_json_pretty_with_defaults(
    data: *mut c_void,
) -> *const c_char {
    cached_json(data, true, true)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_last_json(data: *mut c_void) -> *const c_char {
    borrow_data(data)
        .map(|data| data.json_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_save_json(data: *mut c_void, file: *const c_char) -> bool {
    let Some(text) = write_json(data, file, false) else {
        return false;
    };
    // SAFETY: `file` was checked non-null. The marker is off, as in obs-data.c.
    unsafe { os_quick_write_utf8_file(file, text.as_ptr(), text.as_bytes().len(), false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_save_json_safe(
    data: *mut c_void,
    file: *const c_char,
    temp_ext: *const c_char,
    backup_ext: *const c_char,
) -> bool {
    let Some(text) = write_json(data, file, false) else {
        return false;
    };
    unsafe {
        os_quick_write_utf8_file_safe(
            file,
            text.as_ptr(),
            text.as_bytes().len(),
            false,
            temp_ext,
            backup_ext,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_save_json_pretty_safe(
    data: *mut c_void,
    file: *const c_char,
    temp_ext: *const c_char,
    backup_ext: *const c_char,
) -> bool {
    let Some(text) = write_json(data, file, true) else {
        return false;
    };
    unsafe {
        os_quick_write_utf8_file_safe(
            file,
            text.as_ptr(),
            text.as_bytes().len(),
            false,
            temp_ext,
            backup_ext,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_apply(target: *mut c_void, apply_data: *mut c_void) {
    if let (Some(target), Some(apply_data)) = (borrow_data(target), borrow_data(apply_data)) {
        target.apply(&apply_data);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_erase(data: *mut c_void, name: *const c_char) {
    if let Some(name) = c_str(name)
        && let Some(data) = borrow_data(data)
    {
        data.erase(&name);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_clear(data: *mut c_void) {
    if let Some(data) = borrow_data(data) {
        data.clear();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_defaults(data: *mut c_void) -> *mut c_void {
    let src = borrow_data(data);
    leak_data(Data::defaults_from(src.as_ref()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_create() -> *mut c_void {
    leak_array(Array::new())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_addref(array: *mut c_void) {
    if !array.is_null() {
        unsafe { Arc::increment_strong_count(array as *const RwLock<ArrayInner>) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_release(array: *mut c_void) {
    drop(take_array(array));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_count(array: *mut c_void) -> usize {
    borrow_array(array).map(|array| array.count()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_item(array: *mut c_void, idx: usize) -> *mut c_void {
    borrow_array(array)
        .and_then(|array| array.item(idx))
        .map(leak_data)
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_push_back(array: *mut c_void, obj: *mut c_void) -> usize {
    let Some(array) = borrow_array(array) else {
        return 0;
    };
    let obj = borrow_data(obj);
    array.push(obj.as_ref())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_insert(array: *mut c_void, idx: usize, obj: *mut c_void) {
    if let Some(array) = borrow_array(array) {
        let obj = borrow_data(obj);
        array.insert(idx, obj.as_ref());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_push_back_array(array: *mut c_void, array2: *mut c_void) {
    if let (Some(array), Some(array2)) = (borrow_array(array), borrow_array(array2)) {
        array.push_array(&array2);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_erase(array: *mut c_void, idx: usize) {
    if let Some(array) = borrow_array(array) {
        array.erase(idx);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_array_enum(
    array: *mut c_void,
    cb: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    param: *mut c_void,
) {
    let (Some(array), Some(cb)) = (borrow_array(array), cb) else {
        return;
    };
    array.enum_with(|data| {
        let ptr = Arc::as_ptr(&data.inner) as *mut c_void;
        unsafe { cb(ptr, param) };
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_first(data: *mut c_void) -> *mut c_void {
    borrow_data(data)
        .and_then(|data| data.first())
        .map(leak_item)
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_byname(
    data: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    named(data, name, |data, name| data.item(name))
        .flatten()
        .map(leak_item)
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_next(item: *mut *mut c_void) -> bool {
    if item.is_null() {
        return false;
    }
    let ptr = unsafe { *item };
    let Some(current) = take_item(ptr) else {
        return false;
    };
    match current.next() {
        Some(next) => {
            unsafe { *item = leak_item(next) };
            true
        }
        None => {
            unsafe { *item = ptr::null_mut() };
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_release(item: *mut *mut c_void) {
    if item.is_null() {
        return;
    }
    let ptr = unsafe { *item };
    let Some(current) = take_item(ptr) else {
        return;
    };
    if Arc::strong_count(&current.inner) == 1 {
        unsafe { *item = ptr::null_mut() };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_remove(item: *mut *mut c_void) {
    if item.is_null() {
        return;
    }
    let ptr = unsafe { *item };
    let Some(current) = borrow_item(ptr) else {
        return;
    };
    if current.is_attached() {
        current.remove();
    } else {
        drop(current);
        unsafe { obs_data_item_release(item) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_gettype(item: *mut c_void) -> c_int {
    borrow_item(item)
        .map(|item| item.ty() as c_int)
        .unwrap_or(Type::Null as c_int)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_numtype(item: *mut c_void) -> c_int {
    borrow_item(item)
        .map(|item| item.numtype() as c_int)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_name(item: *mut c_void) -> *const c_char {
    borrow_item(item)
        .map(|item| item.name_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_vec2(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec2,
) {
    set_vec2(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_vec3(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec3,
) {
    set_vec3(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_vec4(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec4,
) {
    set_vec4(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_quat(
    data: *mut c_void,
    name: *const c_char,
    val: *const quat,
) {
    set_quat(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_vec2(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec2,
) {
    set_vec2(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_vec3(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec3,
) {
    set_vec3(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_vec4(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec4,
) {
    set_vec4(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_quat(
    data: *mut c_void,
    name: *const c_char,
    val: *const quat,
) {
    set_quat(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_vec2(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec2,
) {
    set_vec2(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_vec3(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec3,
) {
    set_vec3(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_vec4(
    data: *mut c_void,
    name: *const c_char,
    val: *const vec4,
) {
    set_vec4(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_quat(
    data: *mut c_void,
    name: *const c_char,
    val: *const quat,
) {
    set_quat(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_vec2(data: *mut c_void, name: *const c_char, val: *mut vec2) {
    get_vec2(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_vec3(data: *mut c_void, name: *const c_char, val: *mut vec3) {
    get_vec3(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_vec4(data: *mut c_void, name: *const c_char, val: *mut vec4) {
    get_vec4(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_quat(data: *mut c_void, name: *const c_char, val: *mut quat) {
    get_quat(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_vec2(
    data: *mut c_void,
    name: *const c_char,
    val: *mut vec2,
) {
    get_vec2(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_vec3(
    data: *mut c_void,
    name: *const c_char,
    val: *mut vec3,
) {
    get_vec3(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_vec4(
    data: *mut c_void,
    name: *const c_char,
    val: *mut vec4,
) {
    get_vec4(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_quat(
    data: *mut c_void,
    name: *const c_char,
    val: *mut quat,
) {
    get_quat(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_vec2(
    data: *mut c_void,
    name: *const c_char,
    val: *mut vec2,
) {
    get_vec2(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_vec3(
    data: *mut c_void,
    name: *const c_char,
    val: *mut vec3,
) {
    get_vec3(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_vec4(
    data: *mut c_void,
    name: *const c_char,
    val: *mut vec4,
) {
    get_vec4(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_quat(
    data: *mut c_void,
    name: *const c_char,
    val: *mut quat,
) {
    get_quat(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_frames_per_second(
    data: *mut c_void,
    name: *const c_char,
    fps: media_frames_per_second,
    option: *const c_char,
) {
    set_frames(data, name, fps, option, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_frames_per_second(
    data: *mut c_void,
    name: *const c_char,
    fps: media_frames_per_second,
    option: *const c_char,
) {
    set_frames(data, name, fps, option, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_frames_per_second(
    data: *mut c_void,
    name: *const c_char,
    fps: media_frames_per_second,
    option: *const c_char,
) {
    set_frames(data, name, fps, option, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_frames_per_second(
    data: *mut c_void,
    name: *const c_char,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    get_frames(data, name, fps, option, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_frames_per_second(
    data: *mut c_void,
    name: *const c_char,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    get_frames(data, name, fps, option, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_frames_per_second(
    data: *mut c_void,
    name: *const c_char,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    get_frames(data, name, fps, option, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_frames_per_second(
    item: *mut *mut c_void,
    fps: media_frames_per_second,
    option: *const c_char,
) {
    set_item_frames(item, fps, option, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_frames_per_second(
    item: *mut *mut c_void,
    fps: media_frames_per_second,
    option: *const c_char,
) {
    set_item_frames(item, fps, option, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_frames_per_second(
    item: *mut *mut c_void,
    fps: media_frames_per_second,
    option: *const c_char,
) {
    set_item_frames(item, fps, option, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_frames_per_second(
    item: *mut c_void,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    get_item_frames(item, fps, option, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_frames_per_second(
    item: *mut c_void,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    get_item_frames(item, fps, option, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_frames_per_second(
    item: *mut c_void,
    fps: *mut media_frames_per_second,
    option: *mut *const c_char,
) -> bool {
    get_item_frames(item, fps, option, 2)
}

fn set_str(data: *mut c_void, name: *const c_char, val: *const c_char, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        let val = c_str(val);
        match slot {
            1 => data.set_default_string(&name, val.as_deref()),
            2 => data.set_autoselect_string(&name, val.as_deref()),
            _ => data.set_string(&name, val.as_deref()),
        }
    }
}

fn set_i64(data: *mut c_void, name: *const c_char, val: i64, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        match slot {
            1 => data.set_default_int(&name, val),
            2 => data.set_autoselect_int(&name, val),
            _ => data.set_int(&name, val),
        }
    }
}

fn set_f64(data: *mut c_void, name: *const c_char, val: f64, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        match slot {
            1 => data.set_default_double(&name, val),
            2 => data.set_autoselect_double(&name, val),
            _ => data.set_double(&name, val),
        }
    }
}

fn set_bool(data: *mut c_void, name: *const c_char, val: bool, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        match slot {
            1 => data.set_default_bool(&name, val),
            2 => data.set_autoselect_bool(&name, val),
            _ => data.set_bool(&name, val),
        }
    }
}

fn set_obj(data: *mut c_void, name: *const c_char, obj: *mut c_void, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        let obj = borrow_data(obj);
        match slot {
            1 => data.set_default_obj(&name, obj.as_ref()),
            2 => data.set_autoselect_obj(&name, obj.as_ref()),
            _ => data.set_obj(&name, obj.as_ref()),
        }
    }
}

fn set_arr(data: *mut c_void, name: *const c_char, array: *mut c_void, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        let array = borrow_array(array);
        match slot {
            1 => data.set_default_array(&name, array.as_ref()),
            2 => data.set_autoselect_array(&name, array.as_ref()),
            _ => data.set_array(&name, array.as_ref()),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_string(
    data: *mut c_void,
    name: *const c_char,
    val: *const c_char,
) {
    set_str(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_int(data: *mut c_void, name: *const c_char, val: i64) {
    set_i64(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_double(data: *mut c_void, name: *const c_char, val: f64) {
    set_f64(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_bool(data: *mut c_void, name: *const c_char, val: bool) {
    set_bool(data, name, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_obj(
    data: *mut c_void,
    name: *const c_char,
    obj: *mut c_void,
) {
    set_obj(data, name, obj, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_array(
    data: *mut c_void,
    name: *const c_char,
    array: *mut c_void,
) {
    set_arr(data, name, array, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_string(
    data: *mut c_void,
    name: *const c_char,
    val: *const c_char,
) {
    set_str(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_int(
    data: *mut c_void,
    name: *const c_char,
    val: i64,
) {
    set_i64(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_double(
    data: *mut c_void,
    name: *const c_char,
    val: f64,
) {
    set_f64(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_bool(
    data: *mut c_void,
    name: *const c_char,
    val: bool,
) {
    set_bool(data, name, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_obj(
    data: *mut c_void,
    name: *const c_char,
    obj: *mut c_void,
) {
    set_obj(data, name, obj, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_default_array(
    data: *mut c_void,
    name: *const c_char,
    array: *mut c_void,
) {
    set_arr(data, name, array, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_string(
    data: *mut c_void,
    name: *const c_char,
    val: *const c_char,
) {
    set_str(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_int(
    data: *mut c_void,
    name: *const c_char,
    val: i64,
) {
    set_i64(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_double(
    data: *mut c_void,
    name: *const c_char,
    val: f64,
) {
    set_f64(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_bool(
    data: *mut c_void,
    name: *const c_char,
    val: bool,
) {
    set_bool(data, name, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_obj(
    data: *mut c_void,
    name: *const c_char,
    obj: *mut c_void,
) {
    set_obj(data, name, obj, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_set_autoselect_array(
    data: *mut c_void,
    name: *const c_char,
    array: *mut c_void,
) {
    set_arr(data, name, array, 2);
}

fn get_str(data: *mut c_void, name: *const c_char, slot: u8) -> *const c_char {
    let Some(item) = named(data, name, |data, name| data.item(name)).flatten() else {
        return empty();
    };
    match slot {
        1 => item.default_string_ptr(),
        2 => item.autoselect_string_ptr(),
        _ => item.string_ptr(),
    }
}

fn get_i64(data: *mut c_void, name: *const c_char, slot: u8) -> i64 {
    named(data, name, |data, name| match slot {
        1 => data.get_default_int(name),
        2 => data.get_autoselect_int(name),
        _ => data.get_int(name),
    })
    .unwrap_or(0)
}

fn get_f64(data: *mut c_void, name: *const c_char, slot: u8) -> f64 {
    named(data, name, |data, name| match slot {
        1 => data.get_default_double(name),
        2 => data.get_autoselect_double(name),
        _ => data.get_double(name),
    })
    .unwrap_or(0.0)
}

fn get_bool(data: *mut c_void, name: *const c_char, slot: u8) -> bool {
    named(data, name, |data, name| match slot {
        1 => data.get_default_bool(name),
        2 => data.get_autoselect_bool(name),
        _ => data.get_bool(name),
    })
    .unwrap_or(false)
}

fn get_obj(data: *mut c_void, name: *const c_char, slot: u8) -> *mut c_void {
    named(data, name, |data, name| match slot {
        1 => data.get_default_obj(name),
        2 => data.get_autoselect_obj(name),
        _ => data.get_obj(name),
    })
    .flatten()
    .map(leak_data)
    .unwrap_or(ptr::null_mut())
}

fn get_arr(data: *mut c_void, name: *const c_char, slot: u8) -> *mut c_void {
    named(data, name, |data, name| match slot {
        1 => data.get_default_array(name),
        2 => data.get_autoselect_array(name),
        _ => data.get_array(name),
    })
    .flatten()
    .map(leak_array)
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_string(
    data: *mut c_void,
    name: *const c_char,
) -> *const c_char {
    get_str(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_int(data: *mut c_void, name: *const c_char) -> i64 {
    get_i64(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_double(data: *mut c_void, name: *const c_char) -> f64 {
    get_f64(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_bool(data: *mut c_void, name: *const c_char) -> bool {
    get_bool(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_obj(data: *mut c_void, name: *const c_char) -> *mut c_void {
    get_obj(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_array(data: *mut c_void, name: *const c_char) -> *mut c_void {
    get_arr(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_string(
    data: *mut c_void,
    name: *const c_char,
) -> *const c_char {
    get_str(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_int(data: *mut c_void, name: *const c_char) -> i64 {
    get_i64(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_double(
    data: *mut c_void,
    name: *const c_char,
) -> f64 {
    get_f64(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_bool(data: *mut c_void, name: *const c_char) -> bool {
    get_bool(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_obj(
    data: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    get_obj(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_default_array(
    data: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    get_arr(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_string(
    data: *mut c_void,
    name: *const c_char,
) -> *const c_char {
    get_str(data, name, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_int(
    data: *mut c_void,
    name: *const c_char,
) -> i64 {
    get_i64(data, name, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_double(
    data: *mut c_void,
    name: *const c_char,
) -> f64 {
    get_f64(data, name, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_bool(
    data: *mut c_void,
    name: *const c_char,
) -> bool {
    get_bool(data, name, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_obj(
    data: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    get_obj(data, name, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_get_autoselect_array(
    data: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    get_arr(data, name, 2)
}

fn has_slot(data: *mut c_void, name: *const c_char, slot: u8) -> bool {
    named(data, name, |data, name| match slot {
        1 => data.has_default_value(name),
        2 => data.has_autoselect_value(name),
        _ => data.has_user_value(name),
    })
    .unwrap_or(false)
}

fn unset_slot(data: *mut c_void, name: *const c_char, slot: u8) {
    if let Some((data, name)) = named_pair(data, name) {
        match slot {
            1 => data.unset_default_value(&name),
            2 => data.unset_autoselect_value(&name),
            _ => data.unset_user_value(&name),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_has_user_value(data: *mut c_void, name: *const c_char) -> bool {
    has_slot(data, name, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_unset_user_value(data: *mut c_void, name: *const c_char) {
    unset_slot(data, name, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_has_default_value(
    data: *mut c_void,
    name: *const c_char,
) -> bool {
    has_slot(data, name, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_unset_default_value(data: *mut c_void, name: *const c_char) {
    unset_slot(data, name, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_has_autoselect_value(
    data: *mut c_void,
    name: *const c_char,
) -> bool {
    has_slot(data, name, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_unset_autoselect_value(data: *mut c_void, name: *const c_char) {
    unset_slot(data, name, 2);
}

fn item_set_str(item: *mut *mut c_void, val: *const c_char, slot: u8) {
    let Some(item) = item_slot(item) else {
        return;
    };
    let val = c_str(val);
    match slot {
        1 => item.set_default_string(val.as_deref()),
        2 => item.set_autoselect_string(val.as_deref()),
        _ => item.set_string(val.as_deref()),
    }
}

fn item_set_i64(item: *mut *mut c_void, val: i64, slot: u8) {
    let Some(item) = item_slot(item) else {
        return;
    };
    match slot {
        1 => item.set_default_int(val),
        2 => item.set_autoselect_int(val),
        _ => item.set_int(val),
    }
}

fn item_set_f64(item: *mut *mut c_void, val: f64, slot: u8) {
    let Some(item) = item_slot(item) else {
        return;
    };
    match slot {
        1 => item.set_default_double(val),
        2 => item.set_autoselect_double(val),
        _ => item.set_double(val),
    }
}

fn item_set_bool(item: *mut *mut c_void, val: bool, slot: u8) {
    let Some(item) = item_slot(item) else {
        return;
    };
    match slot {
        1 => item.set_default_bool(val),
        2 => item.set_autoselect_bool(val),
        _ => item.set_bool(val),
    }
}

fn item_set_obj(item: *mut *mut c_void, val: *mut c_void, slot: u8) {
    let Some(item) = item_slot(item) else {
        return;
    };
    let val = borrow_data(val);
    match slot {
        1 => item.set_default_obj(val.as_ref()),
        2 => item.set_autoselect_obj(val.as_ref()),
        _ => item.set_obj(val.as_ref()),
    }
}

fn item_set_arr(item: *mut *mut c_void, val: *mut c_void, slot: u8) {
    let Some(item) = item_slot(item) else {
        return;
    };
    let val = borrow_array(val);
    match slot {
        1 => item.set_default_array(val.as_ref()),
        2 => item.set_autoselect_array(val.as_ref()),
        _ => item.set_array(val.as_ref()),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_string(item: *mut *mut c_void, val: *const c_char) {
    item_set_str(item, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_int(item: *mut *mut c_void, val: i64) {
    item_set_i64(item, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_double(item: *mut *mut c_void, val: f64) {
    item_set_f64(item, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_bool(item: *mut *mut c_void, val: bool) {
    item_set_bool(item, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_obj(item: *mut *mut c_void, val: *mut c_void) {
    item_set_obj(item, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_array(item: *mut *mut c_void, val: *mut c_void) {
    item_set_arr(item, val, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_string(
    item: *mut *mut c_void,
    val: *const c_char,
) {
    item_set_str(item, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_int(item: *mut *mut c_void, val: i64) {
    item_set_i64(item, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_double(item: *mut *mut c_void, val: f64) {
    item_set_f64(item, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_bool(item: *mut *mut c_void, val: bool) {
    item_set_bool(item, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_obj(item: *mut *mut c_void, val: *mut c_void) {
    item_set_obj(item, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_default_array(item: *mut *mut c_void, val: *mut c_void) {
    item_set_arr(item, val, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_string(
    item: *mut *mut c_void,
    val: *const c_char,
) {
    item_set_str(item, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_int(item: *mut *mut c_void, val: i64) {
    item_set_i64(item, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_double(item: *mut *mut c_void, val: f64) {
    item_set_f64(item, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_bool(item: *mut *mut c_void, val: bool) {
    item_set_bool(item, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_obj(
    item: *mut *mut c_void,
    val: *mut c_void,
) {
    item_set_obj(item, val, 2);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_set_autoselect_array(
    item: *mut *mut c_void,
    val: *mut c_void,
) {
    item_set_arr(item, val, 2);
}

fn item_get_str(item: *mut c_void, slot: u8) -> *const c_char {
    let Some(item) = borrow_item(item) else {
        return empty();
    };
    match slot {
        1 => item.default_string_ptr(),
        2 => item.autoselect_string_ptr(),
        _ => item.string_ptr(),
    }
}

fn item_get_i64(item: *mut c_void, slot: u8) -> i64 {
    borrow_item(item)
        .map(|item| match slot {
            1 => item.get_default_int(),
            2 => item.get_autoselect_int(),
            _ => item.get_int(),
        })
        .unwrap_or(0)
}

fn item_get_f64(item: *mut c_void, slot: u8) -> f64 {
    borrow_item(item)
        .map(|item| match slot {
            1 => item.get_default_double(),
            2 => item.get_autoselect_double(),
            _ => item.get_double(),
        })
        .unwrap_or(0.0)
}

fn item_get_bool(item: *mut c_void, slot: u8) -> bool {
    borrow_item(item)
        .map(|item| match slot {
            1 => item.get_default_bool(),
            2 => item.get_autoselect_bool(),
            _ => item.get_bool(),
        })
        .unwrap_or(false)
}

fn item_get_obj(item: *mut c_void, slot: u8) -> *mut c_void {
    borrow_item(item)
        .and_then(|item| match slot {
            1 => item.get_default_obj(),
            2 => item.get_autoselect_obj(),
            _ => item.get_obj(),
        })
        .map(leak_data)
        .unwrap_or(ptr::null_mut())
}

fn item_get_arr(item: *mut c_void, slot: u8) -> *mut c_void {
    borrow_item(item)
        .and_then(|item| match slot {
            1 => item.get_default_array(),
            2 => item.get_autoselect_array(),
            _ => item.get_array(),
        })
        .map(leak_array)
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_string(item: *mut c_void) -> *const c_char {
    item_get_str(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_int(item: *mut c_void) -> i64 {
    item_get_i64(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_double(item: *mut c_void) -> f64 {
    item_get_f64(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_bool(item: *mut c_void) -> bool {
    item_get_bool(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_obj(item: *mut c_void) -> *mut c_void {
    item_get_obj(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_array(item: *mut c_void) -> *mut c_void {
    item_get_arr(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_string(item: *mut c_void) -> *const c_char {
    item_get_str(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_int(item: *mut c_void) -> i64 {
    item_get_i64(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_double(item: *mut c_void) -> f64 {
    item_get_f64(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_bool(item: *mut c_void) -> bool {
    item_get_bool(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_obj(item: *mut c_void) -> *mut c_void {
    item_get_obj(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_default_array(item: *mut c_void) -> *mut c_void {
    item_get_arr(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_string(item: *mut c_void) -> *const c_char {
    item_get_str(item, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_int(item: *mut c_void) -> i64 {
    item_get_i64(item, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_double(item: *mut c_void) -> f64 {
    item_get_f64(item, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_bool(item: *mut c_void) -> bool {
    item_get_bool(item, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_obj(item: *mut c_void) -> *mut c_void {
    item_get_obj(item, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_get_autoselect_array(item: *mut c_void) -> *mut c_void {
    item_get_arr(item, 2)
}

fn item_has(item: *mut c_void, slot: u8) -> bool {
    borrow_item(item)
        .map(|item| match slot {
            1 => item.has_default_value(),
            2 => item.has_autoselect_value(),
            _ => item.has_user_value(),
        })
        .unwrap_or(false)
}

fn item_unset(item: *mut c_void, slot: u8) {
    let Some(item) = borrow_item(item) else {
        return;
    };
    match slot {
        1 => item.unset_default_value(),
        2 => item.unset_autoselect_value(),
        _ => item.unset_user_value(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_has_user_value(item: *mut c_void) -> bool {
    item_has(item, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_unset_user_value(item: *mut c_void) {
    item_unset(item, 0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_has_default_value(item: *mut c_void) -> bool {
    item_has(item, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_unset_default_value(item: *mut c_void) {
    item_unset(item, 1);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_has_autoselect_value(item: *mut c_void) -> bool {
    item_has(item, 2)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_data_item_unset_autoselect_value(item: *mut c_void) {
    item_unset(item, 2);
}
