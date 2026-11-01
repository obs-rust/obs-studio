//! `oracle_*` build of `libobs/audio-monitoring/osx/coreaudio-device-rules.c.h`
//! (oracle/coreaudio_device_rules.c).

use std::ffi::c_char;

unsafe extern "C" {
    pub fn oracle_coreaudio_device_listed(allow_inputs: bool, output_stream_bytes: u32) -> bool;
    pub fn oracle_coreaudio_device_count(first_size: u32, second_size: u32, id_size: usize) -> u32;
    pub fn oracle_coreaudio_device_is_default(id: *const c_char) -> bool;
    pub fn oracle_coreaudio_device_names_match(name1: *const c_char, name2: *const c_char) -> bool;
    pub fn oracle_coreaudio_device_is_input(uid: *const c_char) -> bool;
}
