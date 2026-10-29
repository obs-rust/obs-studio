//! `oracle_*` build of `libobs/obs-module-register-copy.c.h`
//! (oracle/module_register_copy.c).
//!
//! The gate enum is a 32-bit integer, returned as `u32` as the shims do.
//! The constants of `obs-module-register-copy.h` and `obs-source.h`, the
//! 32-bit field lists of `obs-module.c`, the offsets and sizes of the fields
//! the registrations read, and the structure sizes come from the same C
//! file.

/// Index of each entry in `oracle_module_register_copy_constant`, in the
/// C's order.
pub const CONSTANT_NAMES: [&str; 8] = [
    "OBS_MODULE_REGISTER_COPY_COPY",
    "OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD",
    "OBS_MODULE_REGISTER_COPY_TOO_LARGE",
    "OBS_MODULE_REGISTER_COPY_SOURCE_TYPE_TRANSITION",
    "OBS_SOURCE_TYPE_TRANSITION",
    "sizeof(enum obs_module_register_copy_gate)",
    "sizeof(uint32_t)",
    "sizeof(void *)",
];

/// The structure kinds of the field and size queries.
pub const KIND_SOURCE: u32 = 0;
pub const KIND_OUTPUT: u32 = 1;
pub const KIND_ENCODER: u32 = 2;
pub const KIND_SERVICE: u32 = 3;

unsafe extern "C" {
    pub fn oracle_obs_module_register_copy_whole_size(
        size: usize,
        ptr_size: usize,
        u32_fields: *const usize,
        count: usize,
    ) -> usize;
    pub fn oracle_obs_module_register_copy_within(
        offset: usize,
        field_size: usize,
        size: usize,
    ) -> bool;
    pub fn oracle_obs_module_register_copy_gate(size: usize, supported: usize) -> u32;
    pub fn oracle_obs_module_register_copy_ignores_size(source_type: u32) -> bool;
    pub fn oracle_module_register_copy_constant(i: usize) -> u32;
    pub fn oracle_module_register_copy_u32_fields(kind: u32, out: *mut usize, cap: usize) -> usize;
    pub fn oracle_module_register_copy_pointer_fields(
        kind: u32,
        out: *mut usize,
        cap: usize,
    ) -> usize;
    pub fn oracle_module_register_copy_struct_size(kind: u32) -> usize;
}

/// The 32-bit field offsets `obs-module.c` lists for a structure kind.
pub fn u32_fields(kind: u32) -> Vec<usize> {
    let mut out = [0usize; 8];
    // SAFETY: `out` has room for `out.len()` entries.
    let n = unsafe { oracle_module_register_copy_u32_fields(kind, out.as_mut_ptr(), out.len()) };
    out[..n.min(out.len())].to_vec()
}

/// The `(offset, size)` of each field the registrations read for a
/// structure kind.
pub fn read_fields(kind: u32) -> Vec<(usize, usize)> {
    let mut out = [0usize; 32];
    // SAFETY: `out` has room for `out.len()` entries.
    let n =
        unsafe { oracle_module_register_copy_pointer_fields(kind, out.as_mut_ptr(), out.len()) };
    out[..n.min(out.len())]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[offset, size]| (offset, size))
        .collect()
}

/// The size of a structure kind.
pub fn struct_size(kind: u32) -> usize {
    // SAFETY: plain value.
    unsafe { oracle_module_register_copy_struct_size(kind) }
}
