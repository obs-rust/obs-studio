//! `oracle_*` build of `libobs/graphics/shader-param-values.c.h`
//! (oracle/shader_param_values.c): the shader parser's number reads,
//! renamed, with the C `lexer.c`, the C `platform-number.c` and the C
//! library's `strtol` and `strtod`.

use core::ffi::{c_char, c_float, c_int};

unsafe extern "C" {
    pub fn oracle_shader_param_array_count(
        str: *const c_char,
        len: usize,
        count: *mut c_int,
    ) -> bool;
    pub fn oracle_shader_param_int_value(str: *const c_char, negative: bool) -> c_int;
    pub fn oracle_shader_param_float_value(str: *const c_char, negative: bool) -> c_float;
    /// `sizeof(long)` in the oracle's C.
    pub fn oracle_shader_param_values_long_size() -> usize;
}
