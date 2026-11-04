//! `oracle_*` build of `libobs/graphics/effect-parser-params.c.h`
//! (oracle/effect_parser_params.c): the effect parser's parameter
//! declarations, annotations and value assignments, renamed, on the C
//! cf-parser inlines with the C `cf_adderror`, `cf_pass_pair`,
//! `cf_literal_to_str`, `strref_cmp`, lexer, preprocessor, parameter type
//! decisions, number reads and `os_strtod`, with helpers that lex a text into
//! a `struct effect_parser` with that C and free it.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// `ep_parse_param` on a `struct effect_parser *`, owning `type` and
    /// `name` (`bmalloc`'d, from [`oracle_effect_parser_params_strdup`]).
    pub fn oracle_ep_parse_param(
        ep: *mut c_void,
        r#type: *mut c_char,
        name: *mut c_char,
        is_property: bool,
        is_const: bool,
        is_uniform: bool,
    );
    /// `ep_init`, then `cf_parser_parse` of `text`, with `cur_token` at
    /// token `start` (at most the terminator). The token count before the
    /// terminator, or 0 when the parse failed.
    pub fn oracle_effect_parser_params_prepare(
        ep: *mut c_void,
        text: *const c_char,
        start: usize,
    ) -> usize;
    /// The parser's token array (`struct cf_token *`), from its first token.
    pub fn oracle_effect_parser_params_tokens(ep: *mut c_void) -> *mut c_void;
    /// `bstrdup(str)`, on libobs's allocator.
    pub fn oracle_effect_parser_params_strdup(str: *const c_char) -> *mut c_char;
    /// Frees the params and errors, keeping the tokens.
    pub fn oracle_effect_parser_params_clear(ep: *mut c_void);
    /// Frees the params, their storage and the cf parser.
    pub fn oracle_effect_parser_params_free(ep: *mut c_void);
    /// Size, alignment and field offsets of `struct ep_param`, written to
    /// `out`; returns how many.
    pub fn oracle_effect_parser_params_layout(out: *mut usize) -> usize;
}
