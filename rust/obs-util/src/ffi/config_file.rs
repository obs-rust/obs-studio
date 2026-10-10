//! C ABI shim for `libobs/util/config-file.c`.
//!
//! `config_t` is opaque to C callers; here it is a `Box<Config>`.
//!
//! NULL-pointer handling mirrors the C contract where C checks it
//! (`config_open(NULL, ..)`, `config_save(NULL)`, `config_close(NULL)`,
//! `config_open_defaults(NULL, ..)`, `config_save_safe(.., NULL, ..)`).
//! Where C would dereference NULL and crash (a NULL `config_t *` to the
//! getters/setters, a NULL `section` or `name`), the shim returns the
//! type's zero value or does nothing; the cmocka tests never exercise it.

use core::ffi::{CStr, c_char, c_int};

use crate::config_file::{CONFIG_ERROR, CONFIG_SUCCESS, Config, open_type_from_c};

/// `struct config_data` is opaque to C.
#[repr(C)]
pub struct config_data {
    _private: [u8; 0],
}

fn as_config<'a>(c: *mut config_data) -> Option<&'a Config> {
    if c.is_null() {
        None
    } else {
        // SAFETY: `c` is a `Config` we allocated in `config_open*`/`config_create`.
        Some(unsafe { &*(c as *const Config) })
    }
}

fn bytes<'a>(ptr: *const c_char) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` is a C string when non-null.
    Some(unsafe { CStr::from_ptr(ptr) }.to_bytes())
}

fn to_box(c: Config) -> *mut config_data {
    Box::into_raw(Box::new(c)).cast()
}

/// # Safety
///
/// `file` is a C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_create(file: *const c_char) -> *mut config_data {
    let Some(file) = bytes(file) else {
        return core::ptr::null_mut();
    };
    Config::create(file).map_or_else(core::ptr::null_mut, to_box)
}

/// # Safety
///
/// `config` is a writable `config_t **` or null; `file` is a C string or
/// null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_open(
    config: *mut *mut config_data,
    file: *const c_char,
    open_type: c_int,
) -> c_int {
    if config.is_null() {
        return CONFIG_ERROR;
    }
    // SAFETY: `config` is writable.
    unsafe { *config = core::ptr::null_mut() };
    let Some(file) = bytes(file) else {
        return crate::config_file::CONFIG_FILENOTFOUND;
    };
    match Config::open(file, open_type_from_c(open_type)) {
        Ok(c) => {
            // SAFETY: `config` is writable.
            unsafe { *config = to_box(c) };
            CONFIG_SUCCESS
        }
        Err(code) => code,
    }
}

/// # Safety
///
/// `config` is a writable `config_t **` or null; `str` is a C string or
/// null (C treats a null `str` as an empty parse and succeeds).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_open_string(
    config: *mut *mut config_data,
    str_: *const c_char,
) -> c_int {
    if config.is_null() {
        return CONFIG_ERROR;
    }
    let text = bytes(str_).unwrap_or(b"");
    // SAFETY: `config` is writable.
    unsafe { *config = to_box(Config::open_string(text)) };
    CONFIG_SUCCESS
}

/// # Safety
///
/// `config` is a live `config_t *` or null; `file` is a C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_open_defaults(
    config: *mut config_data,
    file: *const c_char,
) -> c_int {
    let Some(c) = as_config(config) else {
        return CONFIG_ERROR;
    };
    let Some(file) = bytes(file) else {
        return crate::config_file::CONFIG_FILENOTFOUND;
    };
    c.open_defaults(file)
}

/// # Safety
///
/// `config` is a live `config_t *` or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_save(config: *mut config_data) -> c_int {
    as_config(config).map_or(CONFIG_ERROR, |c| c.save())
}

/// # Safety
///
/// `config` is a live `config_t *` or null; `temp_ext` and `backup_ext`
/// are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_save_safe(
    config: *mut config_data,
    temp_ext: *const c_char,
    backup_ext: *const c_char,
) -> c_int {
    let Some(c) = as_config(config) else {
        return CONFIG_ERROR;
    };
    c.save_safe(bytes(temp_ext), bytes(backup_ext))
}

/// # Safety
///
/// `config` came from `config_open*`/`config_create` or is null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_close(config: *mut config_data) {
    if config.is_null() {
        return;
    }
    // SAFETY: `config` is a `Config` we allocated; dropped exactly once here.
    unsafe { drop(Box::from_raw(config.cast::<Config>())) };
}

/// # Safety
///
/// `config` is a live `config_t *` or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_num_sections(config: *mut config_data) -> usize {
    as_config(config).map_or(0, |c| c.num_sections())
}

/// # Safety
///
/// `config` is a live `config_t *` or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_section(config: *mut config_data, idx: usize) -> *const c_char {
    as_config(config)
        .and_then(|c| c.section_ptr(idx))
        .unwrap_or(core::ptr::null())
}

fn opt_bytes2<'a, 'b>(section: *const c_char, name: *const c_char) -> Option<(&'a [u8], &'b [u8])> {
    Some((bytes(section)?, bytes(name)?))
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_string(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> *const c_char {
    let Some(c) = as_config(config) else {
        return core::ptr::null();
    };
    let Some((s, n)) = opt_bytes2(section, name) else {
        return core::ptr::null();
    };
    c.string_ptr(s, n).unwrap_or(core::ptr::null())
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_int(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> i64 {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return 0;
    };
    c.get_int(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_uint(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> u64 {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return 0;
    };
    c.get_uint(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_bool(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> bool {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return false;
    };
    c.get_bool(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_double(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> f64 {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return 0.0;
    };
    c.get_double(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_string(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: *const c_char,
) {
    let Some(c) = as_config(config) else {
        return;
    };
    let Some((s, n)) = opt_bytes2(section, name) else {
        return;
    };
    c.set_string(s, n, bytes(value).unwrap_or(b""));
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_int(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: i64,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_int(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_uint(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: u64,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_uint(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_bool(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: bool,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_bool(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_double(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: f64,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_double(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_remove_value(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> bool {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return false;
    };
    c.remove_value(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_default_string(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: *const c_char,
) {
    let Some(c) = as_config(config) else {
        return;
    };
    let Some((s, n)) = opt_bytes2(section, name) else {
        return;
    };
    c.set_default_string(s, n, bytes(value).unwrap_or(b""));
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_default_int(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: i64,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_default_int(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_default_uint(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: u64,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_default_uint(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_default_bool(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: bool,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_default_bool(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_set_default_double(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
    value: f64,
) {
    if let (Some(c), Some((s, n))) = (as_config(config), opt_bytes2(section, name)) {
        c.set_default_double(s, n, value);
    }
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_default_string(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> *const c_char {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return core::ptr::null();
    };
    c.default_string_ptr(s, n).unwrap_or(core::ptr::null())
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_default_int(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> i64 {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return 0;
    };
    c.get_default_int(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_default_uint(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> u64 {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return 0;
    };
    c.get_default_uint(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_default_bool(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> bool {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return false;
    };
    c.get_default_bool(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_get_default_double(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> f64 {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return 0.0;
    };
    c.get_default_double(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_has_user_value(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> bool {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return false;
    };
    c.has_user_value(s, n)
}

/// # Safety
///
/// `config` is a live `config_t *` or null; the args are C strings or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn config_has_default_value(
    config: *mut config_data,
    section: *const c_char,
    name: *const c_char,
) -> bool {
    let Some((c, s, n)) = as_config(config)
        .zip(opt_bytes2(section, name))
        .map(|(c, (s, n))| (c, s, n))
    else {
        return false;
    };
    c.has_default_value(s, n)
}
