//! Thin C ABI for `obs_get_module_allow_disable` (#5183).
//!
//! `EXPORT` in `obs.h`, so the shim keeps its name and libobs exports it
//! (`libobs/cmake/rust-exports.d`). Membership is the generated
//! `obs_in_core_module_list()` (`obs-core-modules.h`, compiled from
//! `OBS::libobs-core-modules` into libobs, and into standalone Rust-static
//! consumers by `test/cmocka/tests.d/module_disable_query_link_support.cmake`);
//! the policy is the existing Rust
//! [`crate::module_load_rules::allow_disable`].

use core::ffi::c_char;

use crate::module_disable_query::{Dependencies, get_module_allow_disable};

unsafe extern "C" {
    fn obs_in_core_module_list(name: *const c_char) -> bool;
}

/// The libobs dependencies, made only by [`obs_get_module_allow_disable`]
/// for its caller's name.
struct Libobs;

impl Dependencies<*const c_char> for Libobs {
    fn in_core_module_list(&mut self, name: *const c_char) -> bool {
        // SAFETY: `name` is obs_get_module_allow_disable()'s own argument,
        // forwarded unchanged; its caller guarantees a valid C string.
        unsafe { obs_in_core_module_list(name) }
    }

    fn allow_disable(&mut self, core_name: bool) -> bool {
        crate::module_load_rules::allow_disable(core_name)
    }
}

/// Whether the module named `name` may be disabled.
///
/// # Safety
/// `name` is a valid NUL-terminated string for the call, as the original
/// `strcmp()` matcher requires (NULL is outside its domain).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_allow_disable(name: *const c_char) -> bool {
    get_module_allow_disable(&mut Libobs, name)
}
