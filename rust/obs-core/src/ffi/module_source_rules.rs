//! C ABI for `libobs/obs-module-source-rules.c.h`.
//!
//! `obs_register_source_s()` in `obs-module.c` calls the five functions
//! inside libobs, for every source type a module (or libobs itself)
//! registers. They are declared in `obs-internal.h` without `EXPORT`, so the
//! shims stay hidden (`libobs/cmake/rust-hidden.txt`).
//!
//! The C passes `enum obs_source_type` and returns `enum
//! obs_module_source_rules_list` and `enum obs_module_source_rules_error`:
//! 32-bit integers on every target libobs builds for (`unsigned int` with
//! GCC and Clang, `int` with MSVC), passed and returned as a `u32` here. A
//! type outside the enum is passed through as its bits. The versioned id is
//! `bmalloc`'d, as the C's `dstr` and `bstrdup` results are, and the caller
//! owns it.

use core::ffi::c_char;
use core::ptr;

use super::module_paths::{bstrdup, bytes};
use crate::module_source_rules::{self, Broken, Callbacks, List};

/// `OBS_MODULE_SOURCE_RULES_LIST_UNKNOWN`.
pub const LIST_UNKNOWN: u32 = 0;
/// `OBS_MODULE_SOURCE_RULES_LIST_NONE`.
pub const LIST_NONE: u32 = 1;
/// `OBS_MODULE_SOURCE_RULES_LIST_INPUT`.
pub const LIST_INPUT: u32 = 2;
/// `OBS_MODULE_SOURCE_RULES_LIST_FILTER`.
pub const LIST_FILTER: u32 = 3;
/// `OBS_MODULE_SOURCE_RULES_LIST_TRANSITION`.
pub const LIST_TRANSITION: u32 = 4;

/// `OBS_MODULE_SOURCE_RULES_OK`.
pub const OK: u32 = 0;
/// `OBS_MODULE_SOURCE_RULES_COMPOSITE_AUDIO`.
pub const COMPOSITE_AUDIO: u32 = 1;
/// `OBS_MODULE_SOURCE_RULES_COMPOSITE_ASYNC`.
pub const COMPOSITE_ASYNC: u32 = 2;
/// `OBS_MODULE_SOURCE_RULES_NO_GET_NAME`.
pub const NO_GET_NAME: u32 = 3;
/// `OBS_MODULE_SOURCE_RULES_NO_GET_WIDTH`.
pub const NO_GET_WIDTH: u32 = 4;
/// `OBS_MODULE_SOURCE_RULES_NO_GET_HEIGHT`.
pub const NO_GET_HEIGHT: u32 = 5;
/// `OBS_MODULE_SOURCE_RULES_NO_AUDIO_RENDER`.
pub const NO_AUDIO_RENDER: u32 = 6;

/// Same contract as C `obs_module_source_rules_list_of`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_source_rules_list_of(source_type: u32) -> u32 {
    match module_source_rules::list_of(source_type) {
        List::Unknown => LIST_UNKNOWN,
        List::None => LIST_NONE,
        List::Input => LIST_INPUT,
        List::Filter => LIST_FILTER,
        List::Transition => LIST_TRANSITION,
    }
}

/// Same contract as C `obs_module_source_rules_size_supported`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_source_rules_size_supported(size: usize, supported: usize) -> bool {
    module_source_rules::size_supported(size, supported)
}

/// Same contract as C `obs_module_source_rules_output_flags`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_source_rules_output_flags(source_type: u32, output_flags: u32) -> u32 {
    module_source_rules::output_flags(source_type, output_flags)
}

/// Same contract as C `obs_module_source_rules_check`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_source_rules_check(
    source_type: u32,
    info_flags: u32,
    flags: u32,
    callbacks: u32,
) -> u32 {
    let callbacks = Callbacks::from_bits(callbacks);
    match module_source_rules::check(source_type, info_flags, flags, callbacks) {
        Ok(()) => OK,
        Err(Broken::CompositeAudio) => COMPOSITE_AUDIO,
        Err(Broken::CompositeAsync) => COMPOSITE_ASYNC,
        Err(Broken::NoGetName) => NO_GET_NAME,
        Err(Broken::NoGetWidth) => NO_GET_WIDTH,
        Err(Broken::NoGetHeight) => NO_GET_HEIGHT,
        Err(Broken::NoAudioRender) => NO_AUDIO_RENDER,
    }
}

/// Same contract as C `obs_module_source_rules_versioned_id`.
///
/// # Safety
///
/// `id` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_source_rules_versioned_id(
    id: *const c_char,
    version: u32,
) -> *mut c_char {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let id = unsafe { bytes(id) };
    module_source_rules::versioned_id(id, version).map_or(ptr::null_mut(), |id| bstrdup(&id))
}
