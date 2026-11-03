//! `oracle_*` build of `libobs/graphics/shader-parser-blocks.c.h`
//! (oracle/shader_parser_blocks.c): the shader parser's sampler-state and
//! struct blocks, renamed, on the C cf-parser inlines with the C
//! `cf_adderror`, `cf_pass_pair`, lexer and preprocessor, with helpers that
//! lex a text into a `struct shader_parser` with that C and free it.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// `sp_parse_sampler_state` on a `struct shader_parser *`.
    pub fn oracle_sp_parse_sampler_state(sp: *mut c_void);
    /// `sp_parse_struct` on a `struct shader_parser *`.
    pub fn oracle_sp_parse_struct(sp: *mut c_void);
    /// `shader_parser_init`, then `cf_parser_parse` of `text`, with
    /// `cur_token` at token `start` (at most the terminator). The token
    /// count before the terminator, or 0 when the parse failed.
    pub fn oracle_shader_parser_blocks_prepare(
        sp: *mut c_void,
        text: *const c_char,
        start: usize,
    ) -> usize;
    /// The index of `cur_token` in the token array.
    pub fn oracle_shader_parser_blocks_position(sp: *mut c_void) -> usize;
    /// Frees the samplers, structs and errors, keeping the tokens.
    pub fn oracle_shader_parser_blocks_clear(sp: *mut c_void);
    /// `shader_parser_free`.
    pub fn oracle_shader_parser_blocks_free(sp: *mut c_void);
    /// Size, alignment and field offsets of `struct shader_var`,
    /// `shader_struct`, `shader_sampler`, `shader_parser` and `cf_token`,
    /// written to `out`; returns how many.
    pub fn oracle_shader_parser_blocks_layout(out: *mut usize) -> usize;
}
