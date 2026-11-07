//! `oracle_*` build of `libobs/graphics/effect-parser-writer.c.h`
//! (oracle/effect_parser_writer.c): the effect parser's shader writer,
//! renamed, with the lookups of `effect-parser-funcs.c.h` it calls, on the
//! C `strref_cmp` and dstr.

use core::ffi::c_void;

unsafe extern "C" {
    /// `ep_makeshaderstring(struct effect_parser *, struct dstr *,
    /// cf_token_array_t *, dstr_array_t *)`.
    pub fn oracle_ep_makeshaderstring(
        ep: *mut c_void,
        shader: *mut c_void,
        shader_call: *mut c_void,
        used_params: *mut c_void,
    );
    /// Size, alignment and field offsets of `struct ep_func`, `ep_param`
    /// (the fields the writer reads), `ep_var` (with the size of
    /// `enum ep_var_type`), `ep_struct`, `ep_sampler` and `dstr`, written
    /// to `out`; returns how many.
    pub fn oracle_effect_parser_writer_layout(out: *mut usize) -> usize;
}
