//! Safe core of `libobs/graphics/effect-parser-entry.c.h`: the effect
//! parser's entry point, `ep_parse()`, with its three static helpers
//! `ep_get_var_specifiers`, `report_invalid_func_keyword` and
//! `ep_parse_other`.
//!
//! [`parse`] adds the graphics backend's name as a preprocessor
//! definition (when there is one), sets the parser's effect, lexes and
//! preprocesses the text, then walks the top-level tokens: a `;` or a token
//! that starts with whitespace is stepped over; `struct`, `technique` and
//! `sampler_state` hand the token to their block parsers; a `{` adds
//! "Unexpected code segment" and skips the braces; anything else is a
//! declaration ([`parse_other`]): its specifiers (`property`, `const`,
//! `uniform`, in any order and repeated), its type and its name, then a
//! function on `(` or a parameter on any other token. Once the text is
//! walked, an error (a warning does not count) fails the parse before the
//! compile step; otherwise the result is the compile step's.
//!
//! Each step is an [`EntryModel`] call, in the C's order and under its
//! condition, so a shim can run every one of them on the C structs in place:
//! the block, parameter and function parsers, the specifier check
//! (`ep_check_for_keyword`, which the function parser shares and keeps in
//! C), the compile step, the backend name and the preprocessor. The tokens
//! are the parser's own `cur_token`, walked through [`Tokens`] with the same
//! `util/cf-parser.h` steps as the blocks ([`crate::shader_parser`]).
//!
//! Characterized, not endorsed, and kept (`test_effect_parser_entry`): an
//! empty text fails before the compile step; a preprocessor error does not
//! stop the declarations after it from being parsed, but fails the parse
//! before the compile step; so does an unexpected `{`, after which the
//! walk goes on from the token past its braces; a repeated specifier is a
//! warning and does not stop the compile step.
//!
//! One intentional difference: the C's `_DEBUG_SHADERS` lines (a
//! developer-only define no build sets) are not written.

use core::ffi::CStr;

use crate::shader_parser::Tokens;

/// The variable specifiers before a declaration's type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Specifiers {
    pub is_property: bool,
    pub is_const: bool,
    pub is_uniform: bool,
}

/// The `struct effect_parser` and graphics calls `ep_parse()` makes, beyond
/// the tokens.
pub trait EntryModel: Tokens {
    /// The backend's name, as the model hands it back to [`Self::add_def`].
    type Name;
    /// `gs_preprocessor_name()`: `None` for `NULL`.
    fn preprocessor_name(&mut self) -> Option<Self::Name>;
    /// `cf_def_init()` of a definition named `name` (its `str` and
    /// `unmerged_str`), then `cf_preprocessor_add_def(&ep->cfp.pp, &def)`.
    fn add_def(&mut self, name: Self::Name);
    /// `ep->effect = effect`.
    fn set_effect(&mut self);
    /// `cf_parser_parse(&ep->cfp, effect_string, file)`.
    fn parse_text(&mut self) -> bool;
    /// `ep->cfp.cur_token != NULL`.
    fn has_token(&self) -> bool;
    /// `ep_parse_struct(ep)`.
    fn parse_struct(&mut self);
    /// `ep_parse_technique(ep)`.
    fn parse_technique(&mut self);
    /// `ep_parse_sampler_state(ep)`.
    fn parse_sampler_state(&mut self);
    /// `ep_check_for_keyword(ep, keyword, val)`.
    fn check_for_keyword(&mut self, keyword: &'static CStr, val: &mut bool) -> i32;
    /// `ep_parse_function(ep, type, name)`, which owns both from then on.
    fn parse_function(&mut self, type_name: Vec<u8>, name: Vec<u8>);
    /// `ep_parse_param(ep, type, name, is_property, is_const, is_uniform)`,
    /// which owns the type and name from then on.
    fn parse_param(&mut self, type_name: Vec<u8>, name: Vec<u8>, specifiers: Specifiers);
    /// `error_data_has_errors(&ep->cfp.error_list)`: an error, not a
    /// warning, in the list.
    fn has_errors(&self) -> bool;
    /// `ep_compile(ep)`.
    fn compile(&mut self) -> bool;
}

/// `ep_get_var_specifiers`: `false` at the end of the text.
pub fn get_var_specifiers(m: &mut impl EntryModel, specifiers: &mut Specifiers) -> bool {
    let _ = (m, specifiers);
    todo!("ep_get_var_specifiers")
}

/// `ep_parse_other`: a declaration, from its first token.
pub fn parse_other(m: &mut impl EntryModel) {
    let _ = m;
    todo!("ep_parse_other")
}

/// `ep_parse`: the result `ep_parse()` returns.
pub fn parse(m: &mut impl EntryModel) -> bool {
    let _ = m;
    todo!("ep_parse")
}
