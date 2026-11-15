//! `oracle_*` build of `libobs/graphics/effect-parser-entry.c.h`
//! (oracle/effect_parser_entry.c): the effect parser's entry point,
//! renamed, on the C block, technique and parameter parsers, cf-parser
//! inlines, lexer and preprocessor, with helpers that set up, write out and
//! free a parser and its effect; and the real-name stand-ins the Rust shim
//! and this oracle both call (oracle/effect_parser_entry_links.c): the
//! backend name, the C specifier check and function parser, and the C
//! preprocessor.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// `bool ep_parse(struct effect_parser *, gs_effect_t *, const char *,
    /// const char *)`.
    pub fn oracle_ep_parse(
        ep: *mut c_void,
        effect: *mut c_void,
        effect_string: *const c_char,
        file: *const c_char,
    ) -> bool;
    /// `ep_init(ep)` and `effect_init(effect)`.
    pub fn oracle_effect_parser_entry_init(ep: *mut c_void, effect: *mut c_void);
    /// `ep_free(ep)` and `effect_free(effect)`.
    pub fn oracle_effect_parser_entry_free(ep: *mut c_void, effect: *mut c_void);
    /// Everything the parse left in the parser and the effect, as a
    /// NUL-terminated text, freed with
    /// [`oracle_effect_parser_entry_dump_free`].
    pub fn oracle_effect_parser_entry_dump(ep: *mut c_void, effect: *mut c_void) -> *mut c_char;
    /// Frees a dump.
    pub fn oracle_effect_parser_entry_dump_free(dump: *mut c_char);
    /// Size, alignment and field offsets of `struct cf_def`, written to
    /// `out`; returns how many.
    pub fn oracle_effect_parser_entry_layout(out: *mut usize) -> usize;
    /// The name `gs_preprocessor_name()` returns on this thread (NULL for
    /// none; a C string that outlives its use), with the call count reset.
    pub fn oracle_effect_parser_entry_set_backend(name: *const c_char);
    /// How many times this thread called `gs_preprocessor_name()` since.
    pub fn oracle_effect_parser_entry_name_calls() -> u32;
}
