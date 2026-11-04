//! `oracle_*` build of `libobs/graphics/effect-parser-passes.c.h`
//! (oracle/effect_parser_passes.c): the effect parser's passes and
//! techniques, renamed, on the C cf-parser inlines with the C `cf_adderror`,
//! `cf_pass_pair`, `strref_cmp`, lexer and preprocessor, with helpers that
//! lex a text into a `struct effect_parser` with that C and free it.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// `ep_parse_technique` on a `struct effect_parser *`.
    pub fn oracle_ep_parse_technique(ep: *mut c_void);
    /// `ep_init`, then `cf_parser_parse` of `text`, with `cur_token` at
    /// token `start` (at most the terminator). The token count before the
    /// terminator, or 0 when the parse failed.
    pub fn oracle_effect_parser_passes_prepare(
        ep: *mut c_void,
        text: *const c_char,
        start: usize,
    ) -> usize;
    /// The parser's token array (`struct cf_token *`), from its first token.
    pub fn oracle_effect_parser_passes_tokens(ep: *mut c_void) -> *mut c_void;
    /// Frees the techniques and errors, keeping the tokens.
    pub fn oracle_effect_parser_passes_clear(ep: *mut c_void);
    /// Frees the techniques, their storage and the cf parser.
    pub fn oracle_effect_parser_passes_free(ep: *mut c_void);
    /// Size, alignment and field offsets of `struct ep_pass`,
    /// `ep_technique` and `cf_token` (with the size of
    /// `enum cf_token_type`), written to `out`; returns how many.
    pub fn oracle_effect_parser_passes_layout(out: *mut usize) -> usize;
}
