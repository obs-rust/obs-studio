//! `oracle_*` build of `libobs/obs-module-type-rules.c.h`
//! (oracle/module_type_rules.c).
//!
//! The error enum is a 32-bit integer, returned as `u32` as the shims do.
//! The constants of `obs-module-type-rules.h`, `obs-output.h` and
//! `obs-encoder.h`, and the field ends and structure sizes the registration
//! reads, come from the same C file.

use core::ffi::c_char;

/// Index of each entry in `oracle_module_type_rules_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 49] = [
    "OBS_MODULE_TYPE_RULES_OUTPUT_VIDEO",
    "OBS_MODULE_TYPE_RULES_OUTPUT_AUDIO",
    "OBS_MODULE_TYPE_RULES_OUTPUT_ENCODED",
    "OBS_MODULE_TYPE_RULES_OUTPUT_SERVICE",
    "OBS_MODULE_TYPE_RULES_OUTPUT_MULTI_TRACK",
    "OBS_MODULE_TYPE_RULES_ENCODER_AUDIO",
    "OBS_MODULE_TYPE_RULES_CAP_PASS_TEXTURE",
    "OBS_MODULE_TYPE_RULES_CAP_SCALING",
    "OBS_OUTPUT_VIDEO",
    "OBS_OUTPUT_AUDIO",
    "OBS_OUTPUT_ENCODED",
    "OBS_OUTPUT_SERVICE",
    "OBS_OUTPUT_MULTI_TRACK",
    "OBS_ENCODER_AUDIO",
    "OBS_ENCODER_VIDEO",
    "OBS_ENCODER_CAP_PASS_TEXTURE",
    "OBS_ENCODER_CAP_SCALING",
    "OBS_MODULE_TYPE_RULES_HAS_GET_NAME",
    "OBS_MODULE_TYPE_RULES_HAS_CREATE",
    "OBS_MODULE_TYPE_RULES_HAS_DESTROY",
    "OBS_MODULE_TYPE_RULES_HAS_START",
    "OBS_MODULE_TYPE_RULES_HAS_STOP",
    "OBS_MODULE_TYPE_RULES_HAS_PROTOCOLS",
    "OBS_MODULE_TYPE_RULES_HAS_ENCODED_PACKET",
    "OBS_MODULE_TYPE_RULES_HAS_RAW_VIDEO",
    "OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO",
    "OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO2",
    "OBS_MODULE_TYPE_RULES_HAS_ENCODE",
    "OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE",
    "OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE2",
    "OBS_MODULE_TYPE_RULES_HAS_GET_FRAME_SIZE",
    "OBS_MODULE_TYPE_RULES_HAS_GET_PROTOCOL",
    "OBS_MODULE_TYPE_RULES_OK",
    "OBS_MODULE_TYPE_RULES_TEXTURE_SCALING",
    "OBS_MODULE_TYPE_RULES_NO_GET_NAME",
    "OBS_MODULE_TYPE_RULES_NO_CREATE",
    "OBS_MODULE_TYPE_RULES_NO_DESTROY",
    "OBS_MODULE_TYPE_RULES_NO_START",
    "OBS_MODULE_TYPE_RULES_NO_STOP",
    "OBS_MODULE_TYPE_RULES_NO_PROTOCOLS",
    "OBS_MODULE_TYPE_RULES_NO_ENCODED_PACKET",
    "OBS_MODULE_TYPE_RULES_NO_RAW_VIDEO",
    "OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO",
    "OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO2",
    "OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE",
    "OBS_MODULE_TYPE_RULES_NO_ENCODE",
    "OBS_MODULE_TYPE_RULES_NO_GET_FRAME_SIZE",
    "OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL",
    "sizeof(enum obs_module_type_rules_error)",
];

unsafe extern "C" {
    pub fn oracle_obs_module_type_rules_output_check(flags: u32, callbacks: u32) -> u32;
    pub fn oracle_obs_module_type_rules_encoder_check(
        encoder_type: u32,
        caps: u32,
        callbacks: u32,
    ) -> u32;
    pub fn oracle_obs_module_type_rules_service_check(callbacks: u32) -> u32;
    pub fn oracle_obs_module_type_rules_protocol_listed(
        protocol: *const c_char,
        protocols: *const *mut c_char,
        num: usize,
    ) -> bool;
    pub fn oracle_module_type_rules_constant_count() -> usize;
    pub fn oracle_module_type_rules_constant(i: usize) -> u32;
    pub fn oracle_module_type_rules_field_end(kind: u32, bit: u32) -> usize;
    pub fn oracle_module_type_rules_other_end(which: u32) -> usize;
    pub fn oracle_module_type_rules_struct_size(kind: u32) -> usize;
}
