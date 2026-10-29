//! C ABI for `libobs/obs-module-register-copy.c.h`.
//!
//! `obs_register_source_s()`, `obs_register_output_s()`,
//! `obs_register_encoder_s()` and `obs_register_service_s()` in
//! `obs-module.c` call the four functions inside libobs, for every type a
//! module registers. They are declared in `obs-module-register-copy.h`
//! without `EXPORT`, so the shims stay hidden (`libobs/cmake/rust-hidden.txt`).
//!
//! Sizes and offsets are `size_t`, a `usize` here. The C returns
//! `enum obs_module_register_copy_gate`: a 32-bit integer on every target
//! libobs builds for (`unsigned int` with GCC and Clang, `int` with MSVC),
//! returned as a `u32` here.

use core::slice;

use crate::module_register_copy::{self, Gate};

/// `OBS_MODULE_REGISTER_COPY_COPY`.
pub const COPY: u32 = 0;
/// `OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD`.
pub const OUTSIDE_LOAD: u32 = 1;
/// `OBS_MODULE_REGISTER_COPY_TOO_LARGE`.
pub const TOO_LARGE: u32 = 2;

/// Same contract as C `obs_module_register_copy_whole_size`.
///
/// Where the C would read a NULL list, this does not: a NULL list has no
/// fields. `obs-module.c` passes NULL only with `count` 0.
///
/// # Safety
///
/// `u32_fields` is null or points to `count` readable `size_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_register_copy_whole_size(
    size: usize,
    ptr_size: usize,
    u32_fields: *const usize,
    count: usize,
) -> usize {
    let fields: &[usize] = if u32_fields.is_null() {
        &[]
    } else {
        // SAFETY: the caller guarantees `count` readable entries.
        unsafe { slice::from_raw_parts(u32_fields, count) }
    };
    module_register_copy::whole_size(size, ptr_size, fields)
}

/// Same contract as C `obs_module_register_copy_within`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_register_copy_within(
    offset: usize,
    field_size: usize,
    size: usize,
) -> bool {
    module_register_copy::within(offset, field_size, size)
}

/// Same contract as C `obs_module_register_copy_gate`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_register_copy_gate(size: usize, supported: usize) -> u32 {
    match module_register_copy::gate(size, supported) {
        Gate::Copy => COPY,
        Gate::OutsideLoad => OUTSIDE_LOAD,
        Gate::TooLarge => TOO_LARGE,
    }
}

/// Same contract as C `obs_module_register_copy_ignores_size`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_register_copy_ignores_size(source_type: u32) -> bool {
    module_register_copy::ignores_size(source_type)
}
