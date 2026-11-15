//! C ABI for `libobs/graphics/effect-parser-entry.c.h`: the effect parser's
//! entry point, `ep_parse()`.
//!
//! `gs_effect_create()` (`graphics.c`) calls [`ep_parse`] on a fresh
//! `ep_init()` parser, then `ep_free()`. It is declared in
//! `graphics/effect-parser.h` without `EXPORT`, so the shim stays hidden in
//! libobs (`libobs/cmake/rust-hidden.d`).
//!
//! The core ([`crate::effect_parser::entry`]) walks `ep->cfp.cur_token` in
//! place (the shader parser shim's [`RawTokens`]), and every step it takes
//! is the call the C makes, on the C structs:
//!
//! - the backend name is the hidden `gs_preprocessor_name()`; a definition
//!   is a `struct cf_def` set up as `cf_def_init()` sets it (all zero), with
//!   its name's `str` and `unmerged_str` the backend name, handed to the
//!   exported `cf_preprocessor_add_def()`;
//! - `cf_parser_parse()` (a `util/cf-parser.h` inline, ported under the
//!   header-inline exemption) is the exported `cf_lexer_lex()` (obs-util's
//!   port) and `cf_preprocess()`, then `cur_token` the preprocessor's first
//!   token (`cf_preprocessor_get_tokens()`);
//! - the struct, sampler-state, technique and parameter parsers are the
//!   obs-graphics shims ([`ep_parse_struct`], [`ep_parse_sampler_state`],
//!   [`ep_parse_technique`], [`ep_parse_param`]), the compile step
//!   [`ep_compile`]; the specifier check and the function parser are the
//!   hidden C `ep_check_for_keyword()` and `ep_parse_function()`
//!   (`effect-parser-funcs.c`, #3623). The type and name a declaration
//!   hands over are `bstrdup_n` copies on libobs's allocator, which those
//!   parsers own, as the C's are;
//! - errors go to the exported `cf_adderror()` and the brace skip to
//!   `cf_pass_pair()`, at the same tokens, through the shader parser's
//!   adapters; `error_data_has_errors()` (a `util/lexer.h` inline) reads
//!   `ep->cfp.error_list` in place.
//!
//! A null `ep` returns false (the C dereferences it).

use core::ffi::{CStr, c_char, c_int, c_void};

use obs_util::ffi::cf_parser::cf_preprocessor;
use obs_util::ffi::cf_tokenizer::{cf_lexer, cf_lexer_lex, cf_token};
use obs_util::ffi::darray::darray;
use obs_util::ffi::lexer::{error_data, error_item, strref};

use crate::effect_parser::entry::{self, EntryModel, Specifiers};
use crate::ffi::effect_parser::{
    ep_compile, ep_parse_param, ep_parse_sampler_state, ep_parse_struct, ep_parse_technique,
};
use crate::ffi::effect_parser_lifecycle::effect_parser;
use crate::ffi::shader_parser::{RawTokens, dup};
use crate::shader_parser::{BlockError, LEX_ERROR, Tokens};

/// `struct cf_def` (`util/cf-lexer.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct cf_def {
    pub name: cf_token,
    /// `cf_token_array_t`.
    pub params: darray,
    /// `cf_token_array_t`.
    pub tokens: darray,
    pub r#macro: bool,
}

unsafe extern "C" {
    fn gs_preprocessor_name() -> *const c_char;
    fn cf_preprocessor_add_def(pp: *mut cf_preprocessor, def: *mut cf_def);
    fn cf_preprocess(pp: *mut cf_preprocessor, lex: *mut cf_lexer, ed: *mut error_data) -> bool;
    fn ep_check_for_keyword(
        ep: *mut effect_parser,
        keyword: *const c_char,
        val: *mut bool,
    ) -> c_int;
    fn ep_parse_function(ep: *mut effect_parser, r#type: *mut c_char, name: *mut c_char);
}

/// The parser `ep_parse()` was handed, with its arguments.
struct Entry {
    tokens: RawTokens,
    ep: *mut effect_parser,
    effect: *mut c_void,
    effect_string: *const c_char,
    file: *const c_char,
}

impl Tokens for Entry {
    type Mark = *mut cf_token;

    fn kind(&self) -> i32 {
        self.tokens.kind()
    }

    fn text(&self) -> &[u8] {
        self.tokens.text()
    }

    fn advance(&mut self) {
        self.tokens.advance();
    }

    fn mark(&self) -> Self::Mark {
        self.tokens.mark()
    }

    fn reset(&mut self, mark: Self::Mark) {
        self.tokens.reset(mark);
    }

    fn add_error(&mut self, error: BlockError) {
        self.tokens.add_error(error);
    }

    fn pass_pair(&mut self) -> bool {
        self.tokens.pass_pair()
    }
}

impl EntryModel for Entry {
    type Name = *const c_char;

    fn preprocessor_name(&mut self) -> Option<*const c_char> {
        // SAFETY: a plain getter.
        let name = unsafe { gs_preprocessor_name() };
        (!name.is_null()).then_some(name)
    }

    fn add_def(&mut self, name: *const c_char) {
        // SAFETY: name is the backend's NUL-terminated name, which outlives
        // the parser (the C points the definition at it the same way).
        let len = unsafe { CStr::from_ptr(name) }.to_bytes().len();
        let s = strref { array: name, len };
        // SAFETY: cf_def_init(): every field of cf_def permits zero.
        let mut def: cf_def = unsafe { core::mem::zeroed() };
        def.name.str = s;
        def.name.unmerged_str = s;
        // SAFETY: an initialized parser's preprocessor; it copies the
        // definition, as it does the C's stack one.
        unsafe { cf_preprocessor_add_def(&raw mut (*self.ep).cfp.pp, &raw mut def) };
    }

    fn set_effect(&mut self) {
        // SAFETY: the shim's contract.
        unsafe { (*self.ep).effect = self.effect };
    }

    fn parse_text(&mut self) -> bool {
        // SAFETY: an initialized parser; the strings are the caller's.
        unsafe {
            let cfp = &raw mut (*self.ep).cfp;
            if !cf_lexer_lex(&raw mut (*cfp).lex, self.effect_string, self.file) {
                return false;
            }
            if !cf_preprocess(
                &raw mut (*cfp).pp,
                &raw mut (*cfp).lex,
                &raw mut (*cfp).error_list,
            ) {
                return false;
            }
            (*cfp).cur_token = (*cfp).pp.tokens.array.cast::<cf_token>();
        }
        true
    }

    fn has_token(&self) -> bool {
        // SAFETY: the shim's contract.
        unsafe { !(*self.ep).cfp.cur_token.is_null() }
    }

    fn parse_struct(&mut self) {
        // SAFETY: the parser at a token of its token array.
        unsafe { ep_parse_struct(self.ep) };
    }

    fn parse_technique(&mut self) {
        // SAFETY: as for parse_struct.
        unsafe { ep_parse_technique(self.ep) };
    }

    fn parse_sampler_state(&mut self) {
        // SAFETY: as for parse_struct.
        unsafe { ep_parse_sampler_state(self.ep) };
    }

    fn check_for_keyword(&mut self, keyword: &'static CStr, val: &mut bool) -> i32 {
        // SAFETY: as for parse_struct, with a static keyword.
        unsafe { ep_check_for_keyword(self.ep, keyword.as_ptr(), val) }
    }

    fn parse_function(&mut self, type_name: Vec<u8>, name: Vec<u8>) {
        // SAFETY: as for parse_struct; the function parser owns the copies.
        unsafe { ep_parse_function(self.ep, dup(&type_name), dup(&name)) };
    }

    fn parse_param(&mut self, type_name: Vec<u8>, name: Vec<u8>, specifiers: Specifiers) {
        // SAFETY: as for parse_struct; the parameter parser owns the copies.
        unsafe {
            ep_parse_param(
                self.ep,
                dup(&type_name),
                dup(&name),
                specifiers.is_property,
                specifiers.is_const,
                specifiers.is_uniform,
            );
        }
    }

    fn has_errors(&self) -> bool {
        // SAFETY: the parser's error list holds num error items.
        unsafe {
            let errors = &(*self.ep).cfp.error_list.errors;
            if errors.num == 0 || errors.array.is_null() {
                return false;
            }
            core::slice::from_raw_parts(errors.array.cast::<error_item>(), errors.num)
                .iter()
                .any(|e| e.level == LEX_ERROR)
        }
    }

    fn compile(&mut self) -> bool {
        // SAFETY: the parsed parser, its effect set.
        unsafe { ep_compile(self.ep) }
    }
}

/// `bool ep_parse(struct effect_parser *ep, gs_effect_t *effect, const
/// char *effect_string, const char *file)`.
///
/// # Safety
///
/// `ep` is null or an `ep_init()` parser; `effect_string` and `file` are
/// null or NUL-terminated (`cf_lexer_lex()`'s contract); `effect` is the
/// effect the compile step fills.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_parse(
    ep: *mut effect_parser,
    effect: *mut c_void,
    effect_string: *const c_char,
    file: *const c_char,
) -> bool {
    if ep.is_null() {
        return false;
    }
    let mut m = Entry {
        // SAFETY: the caller's contract.
        tokens: RawTokens(unsafe { &raw mut (*ep).cfp }),
        ep,
        effect,
        effect_string,
        file,
    };
    entry::parse(&mut m)
}
