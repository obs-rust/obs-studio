//! `oracle_*` build of `libobs/graphics/effect-parser-blocks.c.h`
//! (oracle/effect_parser_blocks.c): the effect parser's struct and
//! sampler-state blocks, renamed, on the C cf-parser inlines with the C
//! `cf_adderror`, `cf_pass_pair`, `dstr_ncat`, lexer and preprocessor, with
//! helpers that lex a text into a `struct effect_parser` with that C and free
//! it.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// `ep_parse_struct` on a `struct effect_parser *`.
    pub fn oracle_ep_parse_struct(ep: *mut c_void);
    /// `ep_parse_sampler_state` on a `struct effect_parser *`.
    pub fn oracle_ep_parse_sampler_state(ep: *mut c_void);
    /// `ep_init`, then `cf_parser_parse` of `text`, with `cur_token` at
    /// token `start` (at most the terminator). The token count before the
    /// terminator, or 0 when the parse failed.
    pub fn oracle_effect_parser_blocks_prepare(
        ep: *mut c_void,
        text: *const c_char,
        start: usize,
    ) -> usize;
    /// The index of `cur_token` in the token array.
    pub fn oracle_effect_parser_blocks_position(ep: *mut c_void) -> usize;
    /// Frees the structs, samplers and errors, keeping the tokens.
    pub fn oracle_effect_parser_blocks_clear(ep: *mut c_void);
    /// Frees the structs, samplers, their storage and the cf parser.
    pub fn oracle_effect_parser_blocks_free(ep: *mut c_void);
    /// Size, alignment and field offsets of `struct ep_var` (with the size
    /// of `enum ep_var_type`), `ep_struct`, `ep_sampler` and
    /// `effect_parser`, written to `out`; returns how many.
    pub fn oracle_effect_parser_blocks_layout(out: *mut usize) -> usize;
}
