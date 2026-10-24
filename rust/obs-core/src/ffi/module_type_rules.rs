//! C ABI for `libobs/obs-module-type-rules.c.h`.
//!
//! `obs_register_output_s()`, `obs_register_encoder_s()` and
//! `obs_register_service_s()` in `obs-module.c` call the four functions
//! inside libobs, for every output, encoder and service type a module
//! registers. They are declared in `obs-module-type-rules.h` without
//! `EXPORT`, so the shims stay hidden (`libobs/cmake/rust-hidden.txt`).
//!
//! The flags, capabilities, type and callback bits are `uint32_t`. The C
//! returns `enum obs_module_type_rules_error`: a 32-bit integer on every
//! target libobs builds for (`unsigned int` with GCC and Clang, `int` with
//! MSVC), returned as a `u32` here.

use core::ffi::c_char;
use core::slice;

use super::module_paths::bytes;
use crate::module_type_rules::{self, Broken, Callbacks};

/// `OBS_MODULE_TYPE_RULES_OK`.
pub const OK: u32 = 0;
/// `OBS_MODULE_TYPE_RULES_TEXTURE_SCALING`.
pub const TEXTURE_SCALING: u32 = 1;
/// `OBS_MODULE_TYPE_RULES_NO_GET_NAME`.
pub const NO_GET_NAME: u32 = 2;
/// `OBS_MODULE_TYPE_RULES_NO_CREATE`.
pub const NO_CREATE: u32 = 3;
/// `OBS_MODULE_TYPE_RULES_NO_DESTROY`.
pub const NO_DESTROY: u32 = 4;
/// `OBS_MODULE_TYPE_RULES_NO_START`.
pub const NO_START: u32 = 5;
/// `OBS_MODULE_TYPE_RULES_NO_STOP`.
pub const NO_STOP: u32 = 6;
/// `OBS_MODULE_TYPE_RULES_NO_PROTOCOLS`.
pub const NO_PROTOCOLS: u32 = 7;
/// `OBS_MODULE_TYPE_RULES_NO_ENCODED_PACKET`.
pub const NO_ENCODED_PACKET: u32 = 8;
/// `OBS_MODULE_TYPE_RULES_NO_RAW_VIDEO`.
pub const NO_RAW_VIDEO: u32 = 9;
/// `OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO`.
pub const NO_RAW_AUDIO: u32 = 10;
/// `OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO2`.
pub const NO_RAW_AUDIO2: u32 = 11;
/// `OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE`.
pub const NO_ENCODE_TEXTURE: u32 = 12;
/// `OBS_MODULE_TYPE_RULES_NO_ENCODE`.
pub const NO_ENCODE: u32 = 13;
/// `OBS_MODULE_TYPE_RULES_NO_GET_FRAME_SIZE`.
pub const NO_GET_FRAME_SIZE: u32 = 14;
/// `OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL`.
pub const NO_GET_PROTOCOL: u32 = 15;

/// The C code of a result.
const fn code(result: Result<(), Broken>) -> u32 {
    match result {
        Ok(()) => OK,
        Err(Broken::TextureScaling) => TEXTURE_SCALING,
        Err(Broken::NoGetName) => NO_GET_NAME,
        Err(Broken::NoCreate) => NO_CREATE,
        Err(Broken::NoDestroy) => NO_DESTROY,
        Err(Broken::NoStart) => NO_START,
        Err(Broken::NoStop) => NO_STOP,
        Err(Broken::NoProtocols) => NO_PROTOCOLS,
        Err(Broken::NoEncodedPacket) => NO_ENCODED_PACKET,
        Err(Broken::NoRawVideo) => NO_RAW_VIDEO,
        Err(Broken::NoRawAudio) => NO_RAW_AUDIO,
        Err(Broken::NoRawAudio2) => NO_RAW_AUDIO2,
        Err(Broken::NoEncodeTexture) => NO_ENCODE_TEXTURE,
        Err(Broken::NoEncode) => NO_ENCODE,
        Err(Broken::NoGetFrameSize) => NO_GET_FRAME_SIZE,
        Err(Broken::NoGetProtocol) => NO_GET_PROTOCOL,
    }
}

/// Same contract as C `obs_module_type_rules_output_check`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_type_rules_output_check(flags: u32, callbacks: u32) -> u32 {
    code(module_type_rules::output_check(
        flags,
        Callbacks::from_bits(callbacks),
    ))
}

/// Same contract as C `obs_module_type_rules_encoder_check`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_type_rules_encoder_check(
    encoder_type: u32,
    caps: u32,
    callbacks: u32,
) -> u32 {
    code(module_type_rules::encoder_check(
        encoder_type,
        caps,
        Callbacks::from_bits(callbacks),
    ))
}

/// Same contract as C `obs_module_type_rules_service_check`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_type_rules_service_check(callbacks: u32) -> u32 {
    code(module_type_rules::service_check(Callbacks::from_bits(
        callbacks,
    )))
}

/// Same contract as C `obs_module_type_rules_protocol_listed`.
///
/// Where the C's `strcmp()` would read a NULL pointer, this does not: a
/// NULL protocol is listed nowhere, a NULL list is empty and a NULL entry
/// matches nothing. `obs-module.c` passes none of them (the protocol comes
/// from `strlist_split()`, the entries from `bstrdup()`, and the list is
/// NULL only with `num` 0).
///
/// # Safety
///
/// `protocol` is null or NUL-terminated; `protocols` is null or points to
/// `num` pointers, each null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_type_rules_protocol_listed(
    protocol: *const c_char,
    protocols: *const *mut c_char,
    num: usize,
) -> bool {
    // SAFETY: the caller guarantees a NUL-terminated string or NULL.
    let Some(protocol) = (unsafe { bytes(protocol) }) else {
        return false;
    };
    if protocols.is_null() {
        return false;
    }
    // SAFETY: the caller guarantees `num` readable entries.
    let list = unsafe { slice::from_raw_parts(protocols, num) };
    module_type_rules::protocol_listed(
        protocol,
        list.iter().filter_map(|&entry| {
            // SAFETY: each entry is NUL-terminated or NULL.
            unsafe { bytes(entry) }
        }),
    )
}
