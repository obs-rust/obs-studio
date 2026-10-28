//! C ABI for `libobs/obs-module-load-rules.c.h`.
//!
//! `obs_open_module()`, `load_module_exports()`, the safe and disabled list
//! checks, `obs_get_module_allow_disable()`, `load_all_callback()`,
//! `obs_load_core_modules()`, `find_core_module()`, `obs_is_core_module()`
//! and `obs_is_legacy_module()` in `obs-module.c` call these inside libobs.
//! They are declared in `obs-module-load-rules.h` without `EXPORT`, so the
//! shims stay hidden (`libobs/cmake/rust-hidden.txt`).
//!
//! Codes are `int`, module types, states and route bits `uint32_t`, list
//! lengths and failure counts `size_t`. The C returns
//! `enum obs_module_load_rules_list`: a 32-bit integer on every target libobs
//! builds for (`unsigned int` with GCC and Clang, `int` with MSVC), returned
//! as a `u32` here.

use core::ffi::c_int;

use crate::module_load_rules;

/// Same contract as C `obs_module_load_rules_arg_missing`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_arg_missing(present: bool) -> bool {
    module_load_rules::arg_missing(present)
}

/// Same contract as C `obs_module_load_rules_export_missing`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_export_missing(found: bool) -> bool {
    module_load_rules::export_missing(found)
}

/// Same contract as C `obs_module_load_rules_version_word`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_version_word(ver: u32) -> u32 {
    module_load_rules::version_word(ver)
}

/// Same contract as C `obs_module_load_rules_version_ok`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_version_ok(ver_word: u32, api_ver: u32) -> bool {
    module_load_rules::version_ok(ver_word, api_ver)
}

/// Same contract as C `obs_module_load_rules_safe_list`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_safe_list(num: usize) -> u32 {
    module_load_rules::safe_list(num).code()
}

/// Same contract as C `obs_module_load_rules_disabled_list`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_disabled_list(num: usize) -> u32 {
    module_load_rules::disabled_list(num).code()
}

/// Same contract as C `obs_module_load_rules_allow_disable`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_allow_disable(core_name: bool) -> bool {
    module_load_rules::allow_disable(core_name)
}

/// Same contract as C `obs_module_load_rules_is_core`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_is_core(module_type: u32) -> bool {
    module_load_rules::is_core(module_type)
}

/// Same contract as C `obs_module_load_rules_is_legacy`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_is_legacy(module_type: u32) -> bool {
    module_load_rules::is_legacy(module_type)
}

/// Same contract as C `obs_module_load_rules_open_route`.
///
/// Where the C would write through a NULL `state`, this does not.
/// `obs-module.c` always passes a local.
///
/// # Safety
///
/// `state` is null or points to a writable `uint32_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_load_rules_open_route(code: c_int, state: *mut u32) -> u32 {
    let route = module_load_rules::open_route(code);
    // SAFETY: the caller guarantees a writable `uint32_t` or NULL.
    if let Some(state) = unsafe { state.as_mut() } {
        *state = route.state();
    }
    route.bits()
}

/// Same contract as C `obs_module_load_rules_init_route`.
///
/// Where the C would write through a NULL `state`, this does not.
/// `obs-module.c` always passes a local.
///
/// # Safety
///
/// `state` is null or points to a writable `uint32_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_load_rules_init_route(ok: bool, state: *mut u32) -> u32 {
    let route = module_load_rules::init_route(ok);
    // SAFETY: the caller guarantees a writable `uint32_t` or NULL.
    if let Some(state) = unsafe { state.as_mut() } {
        *state = route.state();
    }
    route.bits()
}

/// Same contract as C `obs_module_load_rules_count_failure`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_count_failure(has_fail_info: bool) -> bool {
    module_load_rules::count_failure(has_fail_info)
}

/// Same contract as C `obs_module_load_rules_core_loaded`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load_rules_core_loaded(fail_count: usize) -> bool {
    module_load_rules::core_loaded(fail_count)
}
