//! C ABI for `libobs/graphics/effect-parser-blocks.c.h`,
//! `libobs/graphics/effect-parser-passes.c.h`,
//! `libobs/graphics/effect-parser-params.c.h`,
//! `libobs/graphics/effect-parser-writer.c.h` and
//! `libobs/graphics/effect-parser-compile.c.h`: the effect parser's struct
//! and sampler-state blocks, its passes and techniques, its parameter
//! declarations, its shader writer and its compile step.
//!
//! `ep_parse()` (`effect-parser.c`) calls [`ep_parse_struct`],
//! [`ep_parse_sampler_state`] and [`ep_parse_technique`] on the keyword
//! token that starts each block. They are declared in
//! `graphics/effect-parser-blocks.h` and `graphics/effect-parser-passes.h`
//! without `EXPORT`, so the shims stay hidden in libobs
//! (`libobs/cmake/rust-hidden.d`).
//!
//! The core walks `ep->cfp.cur_token` in place (the shader parser shim's
//! [`RawTokens`]), so `ep_parse()` goes on from the token where the block
//! stopped, as before. Its errors go to the exported `cf_adderror()` and its
//! `{ ... }` skips to the exported `cf_pass_pair()`, the calls the C makes,
//! each at the same token. A block that parses is written into
//! `ep->structs` or `ep->samplers` as the C builds it: every string a
//! `bmalloc` copy and every array a darray, grown as `util/darray.h` grows
//! it, all on libobs's allocator, so `ep_free()`'s inline `ep_struct_free()`
//! and `ep_sampler_free()` free them. A struct variable is `ep_var_init()`
//! (all zero) with its type, name and mapping; a struct or sampler is
//! `ep_struct_init()` or `ep_sampler_init()` (all zero, so not `written`)
//! with its name and items.
//!
//! A kept technique is `ep_technique_init()` (all zero) with its name and
//! passes, and each pass `ep_pass_init()` (all zero, so its `pass` is
//! `NULL`) with its name (`NULL` when unnamed) and programs. A program
//! entry is the parser's `struct cf_token` at the core's mark, copied
//! whole, as `da_push_back(*call, ep->cfp.cur_token)` copies it (its
//! strrefs still point into the lexer's text, as the C's do), or the
//! zeroed end token of `cf_token_clear()`. The core only marks tokens and
//! never changes the token array, so the copies are taken once it
//! returns.
//!
//! `ep_parse_other()` calls [`ep_parse_param`] on the token after a
//! declaration's name, declared in `graphics/effect-parser-params.h` without
//! `EXPORT` and hidden the same way. A kept param is `ep_param_init()` with
//! the type and name the caller allocated (the shim keeps those two
//! strings, as the C does), the flags, the array count, the default value
//! and the annotations: `default_val` is built as the C builds it, a string
//! or texture path in one `da_copy_array` (its capacity its length), each
//! `int` or `float` with its own 4-byte `da_push_back_array`; each
//! annotation is `ep_param_init()` with `bmalloc` copies of its type and
//! name. Its `gs_effect_param` pointer is `NULL` (the C leaves it unset
//! until `ep_compile_param` writes it). A dropped param frees the caller's
//! type and name, as `ep_param_free()` does. The value parsers read
//! libobs's `os_strtod` and `cf_literal_to_str` through the private
//! adapters, and the invalid type error goes to `cf_adderror()` with the
//! type, at the same token.
//!
//! The compile step calls [`ep_makeshaderstring`] on
//! each pass's vertex and pixel call, declared in
//! `graphics/effect-parser-writer.h` without `EXPORT` and hidden the same
//! way. The writer reads `ep->params`, `structs`, `funcs` and `samplers`
//! and the call's tokens in place, one field at a time where the C reads
//! it, and sets and clears the items' `written` flags where the C does. It
//! appends to the caller's `struct dstr` as `dstr_cat`, `dstr_cat_dstr` and
//! `dstr_catf` append (the buffer grown as `dstr_ensure_capacity` grows it,
//! with libobs's `brealloc`), and pushes each used param's name as a
//! `dstr_init_copy` into `used_params`. The four
//! lookups it shares with the function parser, which `effect-parser-funcs.c`
//! keeps in C, are the core's ports of them (the same first-match rules),
//! so the shim names no C symbol beyond the allocator adapters.
//!
//! `ep_parse()` calls [`ep_compile`] once the text parsed with no error,
//! declared in `graphics/effect-parser-compile.h` without `EXPORT` and
//! hidden the same way. It fills `ep->effect` in place: each
//! `gs_effect_param`, annotation, technique, pass and pass shader param
//! array grown as `da_resize` grows it (new items zero), each name a
//! `bstrdup` copy and each default value moved out of the parser as
//! `da_move` moves it, on libobs's allocator; each parser param points at
//! its effect param. The shader text is the writer shim's
//! ([`ep_makeshaderstring`]), the effect param lookup the Rust
//! `gs_effect_get_param_by_name`, and the shader creates, the shader param
//! lookup and the three `LOG_ERROR` lines (with the C's literal formats)
//! go through the private adapters `obs_graphics_vertexshader_create`,
//! `obs_graphics_pixelshader_create`,
//! `obs_graphics_shader_get_param_by_name` and
//! `obs_graphics_effect_compile_error`; a shader's error text goes to
//! `cf_adderror()`, and is freed. The `_DEBUG_SHADERS` lines are not
//! written.
//!
//! A null `ep` does nothing, and `ep_compile` with a null `ep` or effect
//! returns false (`ep_parse()` never passes one; the C dereferences it).

use core::ffi::{CStr, c_char, c_int, c_void};
use core::mem::size_of;
use core::ptr::{self, NonNull};

use obs_util::ffi::cf_parser::cf_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;
use obs_util::ffi::dstr::dstr;

use crate::effect_param_types::{self, Route};
use crate::effect_parser::compile::{
    self, CompileLog, CompileModel, EFFECT_PASS, EFFECT_TECHNIQUE, ParamSlot, ShaderSlot,
    ShaderType,
};
use crate::effect_parser::writer::{
    self, CallTokens, Deps, ShaderOut, VarType, VarsOf, WriterParser,
};
use crate::effect_parser::{
    self as core_blocks, INVALID_TYPE, Param, ParamTokens, Pass, ProgramToken, Sampler, Struct,
    Technique,
};
use crate::ffi::effect_lookups::{
    gs_effect, gs_effect_get_param_by_name, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use crate::ffi::effect_parser_lifecycle::effect_parser;
use crate::ffi::effect_passes::pass_shaderparam;
use crate::ffi::shader_param_values::libobs_strtod;
use crate::ffi::shader_parser::{RawTokens, dup, empty_darray, push};
use crate::shader_parser::{CFTOKEN_NONE, LEX_ERROR};

/// `struct ep_var` (`graphics/effect-parser.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_var {
    pub r#type: *mut c_char,
    pub name: *mut c_char,
    pub mapping: *mut c_char,
    /// `enum ep_var_type`.
    pub var_type: c_int,
}

/// `struct ep_struct`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_struct {
    pub name: *mut c_char,
    pub vars: darray,
    pub written: bool,
}

/// `struct ep_sampler`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_sampler {
    pub name: *mut c_char,
    pub states: darray,
    pub values: darray,
    pub written: bool,
}

/// `struct ep_pass`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_pass {
    pub name: *mut c_char,
    /// `cf_token_array_t`.
    pub vertex_program: darray,
    /// `cf_token_array_t`.
    pub fragment_program: darray,
    /// `struct gs_effect_pass *`.
    pub pass: *mut c_void,
}

/// `struct ep_technique`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_technique {
    pub name: *mut c_char,
    /// `DARRAY(struct ep_pass)`.
    pub passes: darray,
}

/// The C `struct ep_struct` the C pushes for `eps`.
fn struct_to_c(eps: &Struct) -> ep_struct {
    let mut out = ep_struct {
        name: dup(&eps.name),
        vars: empty_darray(),
        written: false,
    };
    for var in &eps.vars {
        let c_var = ep_var {
            r#type: dup(&var.type_name),
            name: dup(&var.name),
            mapping: var.mapping.as_deref().map_or(ptr::null_mut(), dup),
            var_type: 0,
        };
        // SAFETY: vars starts empty and is grown only here.
        unsafe { push(&mut out.vars, c_var) };
    }
    out
}

/// The C `struct ep_sampler` the C pushes for `eps`.
fn sampler_to_c(eps: &Sampler) -> ep_sampler {
    let mut out = ep_sampler {
        name: dup(&eps.name),
        states: empty_darray(),
        values: empty_darray(),
        written: false,
    };
    for (state, value) in eps.states.iter().zip(&eps.values) {
        // SAFETY: both start empty and are grown only here.
        unsafe {
            push(&mut out.states, dup(state));
            push(&mut out.values, dup(value));
        }
    }
    out
}

/// The `cf_token_array_t` the C fills for `program`.
///
/// # Safety
///
/// Every mark is a token of the parser's token array, still alive.
unsafe fn program_to_c(program: &[ProgramToken<*mut cf_token>]) -> darray {
    let mut out = empty_darray();
    for token in program {
        let c_token = match *token {
            // SAFETY: the caller's contract; a token is plain data, and the
            // copy shares its strrefs as the C's struct copy does.
            ProgramToken::Token(mark) => unsafe { ptr::read(mark) },
            // SAFETY: cf_token_clear(): every field of cf_token permits zero.
            ProgramToken::End => unsafe { core::mem::zeroed::<cf_token>() },
        };
        // SAFETY: out starts empty and is grown only here.
        unsafe { push(&mut out, c_token) };
    }
    out
}

/// The C `struct ep_pass` the C pushes for `pass`.
///
/// # Safety
///
/// As for [`program_to_c`].
unsafe fn pass_to_c(pass: &Pass<*mut cf_token>) -> ep_pass {
    ep_pass {
        name: pass.name.as_deref().map_or(ptr::null_mut(), dup),
        // SAFETY: the caller's contract.
        vertex_program: unsafe { program_to_c(&pass.vertex_program) },
        // SAFETY: the caller's contract.
        fragment_program: unsafe { program_to_c(&pass.fragment_program) },
        pass: ptr::null_mut(),
    }
}

/// The C `struct ep_technique` the C pushes for `ept`.
///
/// # Safety
///
/// As for [`program_to_c`].
unsafe fn technique_to_c(ept: &Technique<*mut cf_token>) -> ep_technique {
    let mut out = ep_technique {
        name: dup(&ept.name),
        passes: empty_darray(),
    };
    for pass in &ept.passes {
        // SAFETY: the caller's contract; passes starts empty and is grown
        // only here.
        unsafe {
            let c_pass = pass_to_c(pass);
            push(&mut out.passes, c_pass);
        }
    }
    out
}

/// `void ep_parse_struct(struct effect_parser *ep)`.
///
/// # Safety
///
/// `ep` is null or an initialized `effect_parser` whose `cfp.cur_token`
/// points into its terminated token array; `ep->structs` is a valid darray
/// of `struct ep_struct`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_parse_struct(ep: *mut effect_parser) {
    if ep.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let mut tokens = RawTokens(unsafe { &raw mut (*ep).cfp });
    if let Some(eps) = core_blocks::parse_struct(&mut tokens) {
        let c_eps = struct_to_c(&eps);
        // SAFETY: structs is a valid darray of struct ep_struct.
        unsafe { push(&mut (*ep).structs, c_eps) };
    }
}

/// `void ep_parse_sampler_state(struct effect_parser *ep)`.
///
/// # Safety
///
/// As for [`ep_parse_struct`], with `ep->samplers` a valid darray of
/// `struct ep_sampler`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_parse_sampler_state(ep: *mut effect_parser) {
    if ep.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let mut tokens = RawTokens(unsafe { &raw mut (*ep).cfp });
    if let Some(eps) = core_blocks::parse_sampler_state(&mut tokens) {
        let c_eps = sampler_to_c(&eps);
        // SAFETY: samplers is a valid darray of struct ep_sampler.
        unsafe { push(&mut (*ep).samplers, c_eps) };
    }
}

/// `void ep_parse_technique(struct effect_parser *ep)`.
///
/// # Safety
///
/// As for [`ep_parse_struct`], with `ep->techniques` a valid darray of
/// `struct ep_technique`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_parse_technique(ep: *mut effect_parser) {
    if ep.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let mut tokens = RawTokens(unsafe { &raw mut (*ep).cfp });
    if let Some(ept) = core_blocks::parse_technique(&mut tokens) {
        // SAFETY: every mark is a token of ep's token array, which the core
        // did not change; techniques is a valid darray of struct
        // ep_technique.
        unsafe {
            let c_ept = technique_to_c(&ept);
            push(&mut (*ep).techniques, c_ept);
        }
    }
}

/// `struct ep_param` (`graphics/effect-parser.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_param {
    pub r#type: *mut c_char,
    pub name: *mut c_char,
    /// `DARRAY(uint8_t)`.
    pub default_val: darray,
    /// `DARRAY(char *)`.
    pub properties: darray,
    /// `struct gs_effect_param *`.
    pub param: *mut c_void,
    pub is_const: bool,
    pub is_property: bool,
    pub is_uniform: bool,
    pub is_texture: bool,
    pub written: bool,
    pub writeorder: c_int,
    pub array_count: c_int,
    /// `ep_param_array_t`.
    pub annotations: darray,
}

unsafe extern "C" {
    // libobs's allocator and cf parser exports, through the private C
    // adapters (libobs/graphics/obs-graphics-adapter.c.h).
    #[link_name = "obs_graphics_bmalloc"]
    fn bmalloc(size: usize) -> *mut c_void;
    #[link_name = "obs_graphics_bfree"]
    fn bfree(ptr: *mut c_void);
    #[link_name = "obs_graphics_cf_adderror"]
    fn cf_adderror(
        p: *mut cf_parser,
        error: *const c_char,
        level: c_int,
        val1: *const c_char,
        val2: *const c_char,
        val3: *const c_char,
    );
    #[link_name = "obs_graphics_cf_literal_to_str"]
    fn cf_literal_to_str(literal: *const c_char, count: usize) -> *mut c_char;
}

impl ParamTokens for RawTokens {
    fn c_str_byte(&self, i: usize) -> u8 {
        let s = &self.token().str;
        if s.array.is_null() {
            return 0;
        }
        // SAFETY: a token's str points into the lexer's NUL-terminated text,
        // and the core asks for byte i only once every byte before it was not
        // NUL, so i is within that text, as the C's strtol and os_strtod read
        // it.
        unsafe { *s.array.cast::<u8>().add(i) }
    }

    fn strtod(&self, copy: &[u8]) -> f64 {
        libobs_strtod(copy)
    }

    fn literal_to_str(&self) -> Option<Vec<u8>> {
        let s = &self.token().str;
        // SAFETY: the token's text, len bytes of the lexer's text, as the C
        // passes it; the result is a bmalloc'd C string or NULL.
        let str = unsafe { cf_literal_to_str(s.array, s.len) };
        if str.is_null() {
            return None;
        }
        // SAFETY: a NUL-terminated string the adapter allocated, freed here
        // as the C frees it after copying.
        unsafe {
            let bytes = CStr::from_ptr(str).to_bytes().to_vec();
            bfree(str.cast());
            Some(bytes)
        }
    }

    fn add_invalid_type_error(&mut self, type_name: &[u8]) {
        // param->type, as a C string again (it holds no NUL).
        let mut val1 = type_name.to_vec();
        val1.push(0);
        // SAFETY: a valid parser at a valid token; the strings are C strings
        // that outlive the call, as the C passes them.
        unsafe {
            cf_adderror(
                self.0,
                INVALID_TYPE.as_ptr(),
                LEX_ERROR,
                val1.as_ptr().cast(),
                ptr::null(),
                ptr::null(),
            );
        }
    }
}

/// The bytes of a C string, empty for `NULL` (`astrcmp_n` reads a NULL
/// type as no match).
///
/// # Safety
///
/// `s` is null or a NUL-terminated string.
unsafe fn c_bytes(s: *const c_char) -> Vec<u8> {
    if s.is_null() {
        return Vec::new();
    }
    // SAFETY: the caller's contract.
    unsafe { CStr::from_ptr(s) }.to_bytes().to_vec()
}

/// `darray_ensure_capacity` of a byte darray: the capacity doubles, or
/// becomes `new_size` from 0 or when doubling is not enough, the old bytes
/// moved to a new `bmalloc` buffer.
///
/// # Safety
///
/// `da` is a valid byte darray (or empty), allocated with `bmalloc`.
unsafe fn ensure_capacity(da: &mut darray, new_size: usize) {
    if new_size <= da.capacity {
        return;
    }
    let new_cap = if da.capacity == 0 {
        new_size
    } else {
        da.capacity.saturating_mul(2).max(new_size)
    };
    // SAFETY: bmalloc never returns null; the old buffer holds `capacity`
    // bytes and came from bmalloc.
    unsafe {
        let p = bmalloc(new_cap);
        if !da.array.is_null() {
            if da.capacity != 0 {
                ptr::copy_nonoverlapping(da.array.cast::<u8>(), p.cast::<u8>(), da.capacity);
            }
            bfree(da.array);
        }
        da.array = p;
    }
    da.capacity = new_cap;
}

/// `da_push_back_array(da, bytes, len)`: the darray resized by `len`
/// (`darray_resize`), then the bytes at its old end.
///
/// # Safety
///
/// As for [`ensure_capacity`].
unsafe fn push_bytes(da: &mut darray, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let old_num = da.num;
    let new_num = old_num.saturating_add(bytes.len());
    // SAFETY: the caller's contract; the buffer then holds new_num bytes.
    unsafe {
        ensure_capacity(da, new_num);
        ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            da.array.cast::<u8>().add(old_num),
            bytes.len(),
        );
    }
    da.num = new_num;
}

/// The `default_val` the C builds for `param`: a string or texture path in
/// one `da_copy_array`, its bytes and NUL, into the empty darray (so its
/// capacity is its length); each `int` or `float` with its own
/// `da_push_back_array`, 4 bytes at a time. The core keeps the bytes; which
/// of the two the C ran is the value parser `obs_effect_param_route()`
/// picked, from the same type and texture flag.
fn default_val_to_c(param: &Param) -> darray {
    let mut out = empty_darray();
    let text = matches!(
        effect_param_types::route(&param.type_name, param.is_texture),
        Route::Texture | Route::String
    );
    let chunk = if text {
        param.default_val.len().max(1)
    } else {
        size_of::<c_int>()
    };
    for bytes in param.default_val.chunks(chunk) {
        // SAFETY: out starts empty and is grown only here.
        unsafe { push_bytes(&mut out, bytes) };
    }
    out
}

/// The C `struct ep_param` the C pushes for `param`, with `r#type` and
/// `name` as its strings: `ep_param_init` with what the parsers filled in.
/// `param` (the `gs_effect_param`) is `NULL`; the C leaves it unset until
/// `ep_compile_param` writes it.
fn param_to_c(param: &Param, r#type: *mut c_char, name: *mut c_char) -> ep_param {
    let mut out = ep_param {
        r#type,
        name,
        default_val: default_val_to_c(param),
        properties: empty_darray(),
        param: ptr::null_mut(),
        is_const: param.is_const,
        is_property: param.is_property,
        is_uniform: param.is_uniform,
        is_texture: param.is_texture,
        written: false,
        writeorder: 0,
        array_count: param.array_count,
        annotations: empty_darray(),
    };
    for annotation in &param.annotations {
        // cf_copy_token: bstrdup_n copies of the type and name tokens.
        let c_annotation = param_to_c(
            annotation,
            dup(&annotation.type_name),
            dup(&annotation.name),
        );
        // SAFETY: annotations starts empty and is grown only here.
        unsafe { push(&mut out.annotations, c_annotation) };
    }
    out
}

/// `void ep_parse_param(struct effect_parser *ep, char *type, char *name,
/// bool is_property, bool is_const, bool is_uniform)`.
///
/// `ep_parse_other()` calls it on the token after the declaration's name,
/// with the type and name it allocated. A kept param keeps those two
/// strings and is pushed into `ep->params`; a dropped one frees them, as
/// `ep_param_free()` does.
///
/// # Safety
///
/// As for [`ep_parse_struct`], with `ep->params` a valid darray of
/// `struct ep_param`; `type` and `name` are null or `bmalloc`'d C strings,
/// owned by the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_parse_param(
    ep: *mut effect_parser,
    r#type: *mut c_char,
    name: *mut c_char,
    is_property: bool,
    is_const: bool,
    is_uniform: bool,
) {
    if ep.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let (type_bytes, name_bytes) = unsafe { (c_bytes(r#type), c_bytes(name)) };
    // SAFETY: the caller's contract.
    let mut tokens = RawTokens(unsafe { &raw mut (*ep).cfp });
    let kept = core_blocks::parse_param(
        &mut tokens,
        type_bytes,
        name_bytes,
        is_property,
        is_const,
        is_uniform,
    );
    match kept {
        Some(param) => {
            let c_param = param_to_c(&param, r#type, name);
            // SAFETY: params is a valid darray of struct ep_param.
            unsafe { push(&mut (*ep).params, c_param) };
        }
        // SAFETY: the caller's contract: both are the call's to free.
        None => unsafe {
            bfree(r#type.cast());
            bfree(name.cast());
        },
    }
}

/// `struct ep_func` (`graphics/effect-parser.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct ep_func {
    pub name: *mut c_char,
    pub ret_type: *mut c_char,
    pub mapping: *mut c_char,
    pub contents: dstr,
    /// `ep_var_array_t`.
    pub param_vars: darray,
    /// `DARRAY(char *)`.
    pub func_deps: darray,
    /// `DARRAY(char *)`.
    pub struct_deps: darray,
    /// `DARRAY(char *)`.
    pub param_deps: darray,
    /// `DARRAY(char *)`.
    pub sampler_deps: darray,
    pub written: bool,
}

unsafe extern "C" {
    // libobs's brealloc, through the private C adapter
    // (libobs/graphics/obs-graphics-adapter.c.h): dstr_ensure_capacity()
    // grows a string with it.
    #[link_name = "obs_graphics_brealloc"]
    fn brealloc(ptr: *mut c_void, size: usize) -> *mut c_void;
}

/// The bytes of a C string, empty for `NULL` (the C would crash on it).
///
/// # Safety
///
/// `s` is null or a NUL-terminated string that outlives `'a`.
unsafe fn c_text<'a>(s: *const c_char) -> &'a [u8] {
    if s.is_null() {
        return &[];
    }
    // SAFETY: the caller's contract.
    unsafe { CStr::from_ptr(s) }.to_bytes()
}

/// A C mapping: `None` for `NULL`.
///
/// # Safety
///
/// As for [`c_text`].
unsafe fn c_mapping<'a>(s: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller's contract.
    (!s.is_null()).then(|| unsafe { c_text(s) })
}

/// The parser, read and written in place: each method reads the one field
/// the C reads at that point, and the `written` flags are set in the
/// items themselves.
struct CParser(*mut effect_parser);

impl CParser {
    /// Item `i` of one of the parser's arrays.
    ///
    /// # Safety
    ///
    /// `da` is one of the parser's arrays of `T`, holding more than `i`
    /// items.
    unsafe fn item<T>(da: &darray, i: usize) -> *mut T {
        // SAFETY: the caller's contract.
        unsafe { da.array.cast::<T>().add(i) }
    }

    fn ep(&self) -> &effect_parser {
        // SAFETY: the shim's contract: a valid parser, which the writer
        // reaches only through this.
        unsafe { &*self.0 }
    }

    fn param(&self, i: usize) -> &ep_param {
        // SAFETY: the core asks only for i < param_count().
        unsafe { &*Self::item::<ep_param>(&self.ep().params, i) }
    }

    fn strukt(&self, i: usize) -> &ep_struct {
        // SAFETY: the core asks only for i < struct_count().
        unsafe { &*Self::item::<ep_struct>(&self.ep().structs, i) }
    }

    fn sampler(&self, i: usize) -> &ep_sampler {
        // SAFETY: the core asks only for i < sampler_count().
        unsafe { &*Self::item::<ep_sampler>(&self.ep().samplers, i) }
    }

    fn func(&self, i: usize) -> &ep_func {
        // SAFETY: the core asks only for i < func_count().
        unsafe { &*Self::item::<ep_func>(&self.ep().funcs, i) }
    }

    fn var(&self, of: VarsOf, j: usize) -> &ep_var {
        let vars = match of {
            VarsOf::Struct(i) => &self.strukt(i).vars,
            VarsOf::Func(i) => &self.func(i).param_vars,
        };
        // SAFETY: the core asks only for j < var_count(of).
        unsafe { &*Self::item::<ep_var>(vars, j) }
    }

    fn deps(&self, i: usize, deps: Deps) -> &darray {
        let f = self.func(i);
        match deps {
            Deps::Funcs => &f.func_deps,
            Deps::Structs => &f.struct_deps,
            Deps::Params => &f.param_deps,
            Deps::Samplers => &f.sampler_deps,
        }
    }

    /// String `j` of a `DARRAY(char *)`.
    fn name_at(da: &darray, j: usize) -> &[u8] {
        // SAFETY: the core asks only for j < num; each entry is a C string
        // of the parser's.
        unsafe { c_text(*Self::item::<*mut c_char>(da, j)) }
    }
}

impl WriterParser for CParser {
    fn param_count(&self) -> usize {
        self.ep().params.num
    }
    fn param_name(&self, i: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.param(i).name) }
    }
    fn param_type(&self, i: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.param(i).r#type) }
    }
    fn param_is_const(&self, i: usize) -> bool {
        self.param(i).is_const
    }
    fn param_is_uniform(&self, i: usize) -> bool {
        self.param(i).is_uniform
    }
    fn param_array_count(&self, i: usize) -> i32 {
        self.param(i).array_count
    }
    fn param_written(&self, i: usize) -> bool {
        self.param(i).written
    }
    fn set_param_written(&mut self, i: usize, written: bool) {
        // SAFETY: i < param_count(); no reference to the item is alive.
        unsafe { (*Self::item::<ep_param>(&(*self.0).params, i)).written = written };
    }

    fn struct_count(&self) -> usize {
        self.ep().structs.num
    }
    fn struct_name(&self, i: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.strukt(i).name) }
    }
    fn struct_written(&self, i: usize) -> bool {
        self.strukt(i).written
    }
    fn set_struct_written(&mut self, i: usize, written: bool) {
        // SAFETY: i < struct_count(); no reference to the item is alive.
        unsafe { (*Self::item::<ep_struct>(&(*self.0).structs, i)).written = written };
    }

    fn sampler_count(&self) -> usize {
        self.ep().samplers.num
    }
    fn sampler_name(&self, i: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.sampler(i).name) }
    }
    fn sampler_value_count(&self, i: usize) -> usize {
        self.sampler(i).values.num
    }
    fn sampler_state(&self, i: usize, j: usize) -> &[u8] {
        let states = &self.sampler(i).states;
        // The parser pushes a state with each value; past its states the C
        // reads out of bounds.
        if j >= states.num {
            return &[];
        }
        Self::name_at(states, j)
    }
    fn sampler_value(&self, i: usize, j: usize) -> &[u8] {
        Self::name_at(&self.sampler(i).values, j)
    }
    fn sampler_written(&self, i: usize) -> bool {
        self.sampler(i).written
    }
    fn set_sampler_written(&mut self, i: usize, written: bool) {
        // SAFETY: i < sampler_count(); no reference to the item is alive.
        unsafe { (*Self::item::<ep_sampler>(&(*self.0).samplers, i)).written = written };
    }

    fn func_count(&self) -> usize {
        self.ep().funcs.num
    }
    fn func_name(&self, i: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.func(i).name) }
    }
    fn func_ret_type(&self, i: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.func(i).ret_type) }
    }
    fn func_mapping(&self, i: usize) -> Option<&[u8]> {
        // SAFETY: null or a C string of the parser's.
        unsafe { c_mapping(self.func(i).mapping) }
    }
    fn func_contents(&self, i: usize) -> &[u8] {
        let contents = &self.func(i).contents;
        if contents.array.is_null() || contents.len == 0 {
            return &[];
        }
        // SAFETY: a dstr holds len bytes.
        unsafe { core::slice::from_raw_parts(contents.array.cast::<u8>(), contents.len) }
    }
    fn dep_count(&self, i: usize, deps: Deps) -> usize {
        self.deps(i, deps).num
    }
    fn dep(&self, i: usize, deps: Deps, k: usize) -> &[u8] {
        Self::name_at(self.deps(i, deps), k)
    }
    fn func_written(&self, i: usize) -> bool {
        self.func(i).written
    }
    fn set_func_written(&mut self, i: usize, written: bool) {
        // SAFETY: i < func_count(); no reference to the item is alive.
        unsafe { (*Self::item::<ep_func>(&(*self.0).funcs, i)).written = written };
    }

    fn var_count(&self, of: VarsOf) -> usize {
        match of {
            VarsOf::Struct(i) => self.strukt(i).vars.num,
            VarsOf::Func(i) => self.func(i).param_vars.num,
        }
    }
    fn var_type(&self, of: VarsOf, j: usize) -> VarType {
        match self.var(of, j).var_type {
            1 => VarType::InOut,
            2 => VarType::Out,
            3 => VarType::Uniform,
            _ => VarType::In,
        }
    }
    fn var_type_name(&self, of: VarsOf, j: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.var(of, j).r#type) }
    }
    fn var_name(&self, of: VarsOf, j: usize) -> &[u8] {
        // SAFETY: a C string of the parser's.
        unsafe { c_text(self.var(of, j).name) }
    }
    fn var_mapping(&self, of: VarsOf, j: usize) -> Option<&[u8]> {
        // SAFETY: null or a C string of the parser's.
        unsafe { c_mapping(self.var(of, j).mapping) }
    }
}

/// A pass call's token array, read a token at a time. The array is not
/// null.
struct CCall(*const darray);

impl CCall {
    fn token(&self, k: usize) -> &cf_token {
        // SAFETY: the shim's contract: a valid token array; the core asks
        // for token k only once every token before it was not the end, and
        // is_end stops at num.
        unsafe { &*(*self.0).array.cast::<cf_token>().add(k) }
    }
}

impl CallTokens for CCall {
    fn is_end(&self, k: usize) -> bool {
        // SAFETY: as for token().
        let num = unsafe { (*self.0).num };
        // The C reads on past num until it finds the end token; the pass
        // parser always ends a call with one.
        k >= num || self.token(k).r#type == CFTOKEN_NONE
    }

    fn lead(&self, k: usize) -> u8 {
        let s = &self.token(k).str;
        if s.array.is_null() {
            return 0;
        }
        // SAFETY: a token's str points into the lexer's text, and the C
        // reads its first byte even when len is 0.
        unsafe { *s.array.cast::<u8>() }
    }

    fn text(&self, k: usize) -> &[u8] {
        let s = &self.token(k).str;
        if s.array.is_null() || s.len == 0 {
            return &[];
        }
        // SAFETY: len bytes of the lexer's text.
        unsafe { core::slice::from_raw_parts(s.array.cast::<u8>(), s.len) }
    }
}

/// `dstr_ensure_capacity`: the capacity doubles, or becomes `new_size`
/// from 0 or when doubling is not enough, with `brealloc`.
///
/// # Safety
///
/// `d` is a valid `struct dstr`, its buffer (if any) from libobs's
/// allocator.
unsafe fn dstr_reserve(d: &mut dstr, new_size: usize) {
    if new_size <= d.capacity {
        return;
    }
    let new_cap = if d.capacity == 0 {
        new_size
    } else {
        d.capacity.saturating_mul(2).max(new_size)
    };
    // SAFETY: brealloc keeps the old bytes and never returns null.
    d.array = unsafe { brealloc(d.array.cast(), new_cap) }.cast();
    d.capacity = new_cap;
}

/// `len` bytes at the end of `d`, then a NUL (`dstr_ncat` and
/// `dstr_cat_dstr` once they append).
///
/// # Safety
///
/// As for [`dstr_reserve`]; `bytes` is not in `d`'s buffer.
unsafe fn dstr_append(d: &mut dstr, bytes: &[u8]) {
    let new_len = d.len.saturating_add(bytes.len());
    // SAFETY: the caller's contract; the buffer then holds new_len + 1
    // bytes.
    unsafe {
        dstr_reserve(d, new_len.saturating_add(1));
        ptr::copy_nonoverlapping(bytes.as_ptr(), d.array.cast::<u8>().add(d.len), bytes.len());
        *d.array.add(new_len) = 0;
    }
    d.len = new_len;
}

/// The caller's shader string and `used_params` list.
struct CShader {
    shader: *mut dstr,
    used_params: *mut darray,
}

impl ShaderOut for CShader {
    fn cat(&mut self, piece: &[u8]) {
        // dstr_ncat: nothing for an empty piece or a NUL first.
        if piece.first().is_none_or(|&b| b == 0) {
            return;
        }
        // SAFETY: the shim's contract: a valid shader string, which no
        // piece points into.
        unsafe { dstr_append(&mut *self.shader, piece) };
    }

    fn cat_dstr(&mut self, piece: &[u8]) {
        if piece.is_empty() {
            return;
        }
        // SAFETY: as for cat.
        unsafe { dstr_append(&mut *self.shader, piece) };
    }

    fn push_used(&mut self, name: &[u8]) {
        // dstr_init_copy: an empty name stays the empty dstr (NULL, 0, 0),
        // as dstr_copy() frees for it.
        let mut new = dstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        if !name.is_empty() {
            // SAFETY: new is empty and grown only here.
            unsafe { dstr_append(&mut new, name) };
        }
        // SAFETY: the shim's contract: a valid darray of struct dstr.
        unsafe { push(&mut *self.used_params, new) };
    }
}

/// `void ep_makeshaderstring(struct effect_parser *ep, struct dstr
/// *shader, cf_token_array_t *shader_call, dstr_array_t *used_params)`.
///
/// The compile step calls it on each pass's vertex and pixel call. It
/// appends to `shader` as the C appends, pushes a copy of each uniform
/// param's name it declares into `used_params` (the caller frees them), and
/// sets and then clears the `written` flags of `ep`'s params, structs,
/// functions and samplers in place, each where the C sets it. The writer
/// reads the parser and the call one field at a time, where the C reads
/// them: a call with no token array, only whitespace or `NULL` reads
/// nothing of the parser and changes nothing. The call's tokens are read up
/// to its end token or its last token, whichever comes first (the C reads
/// on past the last until it finds an end token).
///
/// # Safety
///
/// `ep` is a valid parser as `ep_parse()` leaves it; `shader` a valid
/// `struct dstr` and `used_params` a valid darray of `struct dstr`, both on
/// libobs's allocator; `shader_call` a valid `cf_token_array_t`. Null
/// pointers do nothing (the C dereferences them).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_makeshaderstring(
    ep: *mut effect_parser,
    shader: *mut dstr,
    shader_call: *mut darray,
    used_params: *mut darray,
) {
    if ep.is_null() || shader.is_null() || shader_call.is_null() || used_params.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let call = (!unsafe { (*shader_call).array }.is_null()).then_some(CCall(shader_call));
    let mut out = CShader {
        shader,
        used_params,
    };
    writer::make_shader_string(&mut CParser(ep), &mut out, call.as_ref());
}

// ---------------------------------------------------------------------------
// The compile step (effect-parser-compile.c).

unsafe extern "C" {
    // The graphics device and the compile step's log lines, through the
    // private C adapters (libobs/graphics/obs-graphics-adapter.c.h).
    #[link_name = "obs_graphics_vertexshader_create"]
    fn gs_vertexshader_create(
        shader: *const c_char,
        file: *const c_char,
        error_string: *mut *mut c_char,
    ) -> *mut c_void;
    #[link_name = "obs_graphics_pixelshader_create"]
    fn gs_pixelshader_create(
        shader: *const c_char,
        file: *const c_char,
        error_string: *mut *mut c_char,
    ) -> *mut c_void;
    #[link_name = "obs_graphics_shader_get_param_by_name"]
    fn gs_shader_get_param_by_name(shader: *mut c_void, name: *const c_char) -> *mut c_void;
    /// `blog(LOG_ERROR, ...)` with the C's literal formats.
    fn obs_graphics_effect_compile_error(which: c_int, idx: usize, name: *const c_char);
}

/// `darray_resize` on libobs's allocator: the capacity grows as
/// `darray_ensure_capacity` grows it (the old capacity's bytes copied into
/// a new `bmalloc` buffer), and new items are zero.
///
/// # Safety
///
/// `da` is a valid darray of `elem`-byte items on libobs's allocator.
unsafe fn da_resize(da: &mut darray, elem: usize, num: usize) {
    if num == da.num {
        return;
    } else if num == 0 {
        da.num = 0;
        return;
    }
    let old_num = da.num;
    if num > da.capacity {
        let new_cap = if da.capacity == 0 {
            num
        } else {
            da.capacity.saturating_mul(2).max(num)
        };
        // SAFETY: a plain allocation; the old buffer holds `capacity`
        // items and the new one more, and they are distinct.
        unsafe {
            let p = bmalloc(elem.saturating_mul(new_cap));
            if !da.array.is_null() {
                if da.capacity != 0 {
                    ptr::copy_nonoverlapping(
                        da.array.cast::<u8>(),
                        p.cast::<u8>(),
                        elem.saturating_mul(da.capacity),
                    );
                }
                bfree(da.array);
            }
            da.array = p;
        }
        da.capacity = new_cap;
    }
    da.num = num;
    if num > old_num {
        // SAFETY: the buffer holds `num` items.
        unsafe {
            ptr::write_bytes(
                da.array.cast::<u8>().add(elem.saturating_mul(old_num)),
                0,
                elem.saturating_mul(num - old_num),
            );
        }
    }
}

/// `bstrdup`: a `bmalloc` copy of a C string, `NULL` for `NULL`.
///
/// # Safety
///
/// `s` is null or a NUL-terminated string.
unsafe fn bstrdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller's contract; the copy holds the bytes and the NUL.
    unsafe {
        let bytes = CStr::from_ptr(s).to_bytes_with_nul();
        let dup = bmalloc(bytes.len()).cast::<u8>();
        ptr::copy_nonoverlapping(bytes.as_ptr(), dup, bytes.len());
        dup.cast()
    }
}

/// The shader text and used params one call's `ep_makeshaderstring()`
/// built, as the C's locals.
struct CBuilt {
    shader_str: dstr,
    used_params: darray,
}

/// The parser and its effect, written in place.
struct CCompile {
    ep: *mut effect_parser,
    effect: *mut gs_effect,
}

impl CCompile {
    /// Item `i` of a darray of `T`.
    ///
    /// # Safety
    ///
    /// `da` holds more than `i` items of `T`.
    unsafe fn at<T>(da: &darray, i: usize) -> *mut T {
        // SAFETY: the caller's contract.
        unsafe { da.array.cast::<T>().add(i) }
    }

    fn ep(&self) -> &effect_parser {
        // SAFETY: the shim's contract: a valid parser.
        unsafe { &*self.ep }
    }

    fn effect(&self) -> &gs_effect {
        // SAFETY: the shim's contract: the parser's effect, valid.
        unsafe { &*self.effect }
    }

    fn effect_mut(&mut self) -> &mut gs_effect {
        // SAFETY: as effect(), with exclusive access during the call.
        unsafe { &mut *self.effect }
    }

    /// The effect's param or annotation of `slot` and the parser's.
    fn param_pair(&self, slot: ParamSlot) -> (*mut gs_effect_param, *mut ep_param) {
        // SAFETY: the core asks only for params and annotations within the
        // counts it sized the effect's arrays to, the parser's counts.
        unsafe {
            match slot {
                ParamSlot::Param(i) => (
                    Self::at(&self.effect().params, i),
                    Self::at(&self.ep().params, i),
                ),
                ParamSlot::Annotation { param, index } => {
                    let dst: *mut gs_effect_param = Self::at(&self.effect().params, param);
                    let src: *mut ep_param = Self::at(&self.ep().params, param);
                    (
                        Self::at(&(*dst).annotations, index),
                        Self::at(&(*src).annotations, index),
                    )
                }
            }
        }
    }

    fn technique(&self, t: usize) -> *mut gs_effect_technique {
        // SAFETY: t is within the size the core gave the techniques.
        unsafe { Self::at(&self.effect().techniques, t) }
    }

    fn technique_in(&self, t: usize) -> *mut ep_technique {
        // SAFETY: t is within the parser's techniques.
        unsafe { Self::at(&self.ep().techniques, t) }
    }

    fn pass(&self, t: usize, p: usize) -> *mut gs_effect_pass {
        // SAFETY: p is within the size the core gave the passes.
        unsafe { Self::at(&(*self.technique(t)).passes, p) }
    }

    fn pass_in(&self, t: usize, p: usize) -> *mut ep_pass {
        // SAFETY: p is within the parser's passes of technique t.
        unsafe { Self::at(&(*self.technique_in(t)).passes, p) }
    }

    /// The slot's `pass_shaderparam_array_t`.
    fn shader_params(&self, slot: ShaderSlot) -> *mut darray {
        let pass = self.pass(slot.tech, slot.pass);
        // SAFETY: a valid pass of the effect.
        unsafe {
            match slot.kind {
                ShaderType::Vertex => &raw mut (*pass).vertshader_params,
                ShaderType::Pixel => &raw mut (*pass).pixelshader_params,
            }
        }
    }

    /// `used_params->array[i].array`.
    fn used_name(built: &CBuilt, i: usize) -> *const c_char {
        // SAFETY: the core asks only for i below used_params.num.
        unsafe { (*Self::at::<dstr>(&built.used_params, i)).array }
    }
}

impl CompileModel for CCompile {
    type Shader = NonNull<c_void>;
    type Built = CBuilt;
    type Errors = *mut c_char;
    type EParam = *mut gs_effect_param;
    type SParam = NonNull<c_void>;

    fn param_count(&self) -> usize {
        self.ep().params.num
    }
    fn technique_count(&self) -> usize {
        self.ep().techniques.num
    }
    fn resize_params(&mut self, num: usize) {
        // SAFETY: the effect's DARRAY(struct gs_effect_param).
        unsafe {
            da_resize(
                &mut self.effect_mut().params,
                size_of::<gs_effect_param>(),
                num,
            )
        }
    }
    fn resize_techniques(&mut self, num: usize) {
        // SAFETY: the effect's DARRAY(struct gs_effect_technique).
        unsafe {
            da_resize(
                &mut self.effect_mut().techniques,
                size_of::<gs_effect_technique>(),
                num,
            );
        }
    }
    fn link_param(&mut self, i: usize) {
        let (dst, src) = self.param_pair(ParamSlot::Param(i));
        // SAFETY: valid items of both arrays.
        unsafe { (*src).param = dst.cast() };
    }
    fn param_type_name(&self, slot: ParamSlot) -> &[u8] {
        let (_, src) = self.param_pair(slot);
        // SAFETY: the parser's type, a C string (never NULL).
        unsafe { c_text((*src).r#type) }
    }
    fn fill_param(&mut self, slot: ParamSlot, section: i32, type_: i32) {
        let (dst, src) = self.param_pair(slot);
        let effect = self.effect;
        // SAFETY: valid, distinct items of the two arrays; the name is a C
        // string or NULL; the default value moves as da_move moves it.
        unsafe {
            let dst = &mut *dst;
            let src = &mut *src;
            dst.name = bstrdup(src.name);
            dst.section = section;
            dst.effect = effect;
            bfree(dst.default_val.array);
            dst.default_val = core::mem::replace(&mut src.default_val, empty_darray());
            dst.type_ = type_;
        }
    }
    fn param_name(&self, i: usize) -> &[u8] {
        let (_, src) = self.param_pair(ParamSlot::Param(i));
        // SAFETY: the parser's name, a C string (never NULL).
        unsafe { c_text((*src).name) }
    }
    fn set_view_proj(&mut self, i: usize) {
        let (dst, _) = self.param_pair(ParamSlot::Param(i));
        self.effect_mut().view_proj = dst;
    }
    fn set_world(&mut self, i: usize) {
        let (dst, _) = self.param_pair(ParamSlot::Param(i));
        self.effect_mut().world = dst;
    }
    fn annotation_count(&self, i: usize) -> usize {
        let (_, src) = self.param_pair(ParamSlot::Param(i));
        // SAFETY: a valid parser param.
        unsafe { (*src).annotations.num }
    }
    fn resize_annotations(&mut self, i: usize, num: usize) {
        let (dst, _) = self.param_pair(ParamSlot::Param(i));
        // SAFETY: the param's gs_effect_param_array_t.
        unsafe { da_resize(&mut (*dst).annotations, size_of::<gs_effect_param>(), num) }
    }
    fn fill_technique(&mut self, t: usize) {
        let (dst, src, effect) = (self.technique(t), self.technique_in(t), self.effect);
        // SAFETY: valid items; the name is a C string.
        unsafe {
            (*dst).name = bstrdup((*src).name);
            (*dst).section = EFFECT_TECHNIQUE;
            (*dst).effect = effect;
        }
    }
    fn parser_pass_count(&self, t: usize) -> usize {
        // SAFETY: a valid parser technique.
        unsafe { (*self.technique_in(t)).passes.num }
    }
    fn resize_passes(&mut self, t: usize, num: usize) {
        // SAFETY: the technique's DARRAY(struct gs_effect_pass).
        unsafe {
            da_resize(
                &mut (*self.technique(t)).passes,
                size_of::<gs_effect_pass>(),
                num,
            );
        }
    }
    fn pass_count(&self, t: usize) -> usize {
        // SAFETY: a valid technique.
        unsafe { (*self.technique(t)).passes.num }
    }
    fn fill_pass(&mut self, t: usize, p: usize) {
        let (dst, src) = (self.pass(t, p), self.pass_in(t, p));
        // SAFETY: valid items; the name is a C string or NULL.
        unsafe {
            (*dst).name = bstrdup((*src).name);
            (*dst).section = EFFECT_PASS;
        }
    }
    fn pass_name(&self, t: usize, p: usize) -> Option<&[u8]> {
        // SAFETY: a valid pass; its name a C string or NULL.
        unsafe { c_mapping((*self.pass(t, p)).name) }
    }
    fn technique_name(&self, t: usize) -> Option<&[u8]> {
        // SAFETY: a valid technique; its name a C string or NULL.
        unsafe { c_mapping((*self.technique(t)).name) }
    }
    fn file(&self) -> Option<&[u8]> {
        // SAFETY: the lexer's file name, a C string or NULL.
        unsafe { c_mapping(self.ep().cfp.lex.file) }
    }
    fn make_shader(&mut self, slot: ShaderSlot) -> CBuilt {
        let mut built = CBuilt {
            shader_str: dstr {
                array: ptr::null_mut(),
                len: 0,
                capacity: 0,
            },
            used_params: empty_darray(),
        };
        let pass_in = self.pass_in(slot.tech, slot.pass);
        // SAFETY: the parser, the pass's program (a cf_token_array_t) and
        // the two fresh locals, as the C passes them.
        unsafe {
            let call = match slot.kind {
                ShaderType::Vertex => &raw mut (*pass_in).vertex_program,
                ShaderType::Pixel => &raw mut (*pass_in).fragment_program,
            };
            ep_makeshaderstring(self.ep, &mut built.shader_str, call, &mut built.used_params);
        }
        built
    }
    fn create_shader(
        &mut self,
        slot: ShaderSlot,
        built: &CBuilt,
        location: &[u8],
    ) -> (Option<NonNull<c_void>>, *mut c_char) {
        let mut location = location.to_vec();
        location.push(0);
        let mut errors: *mut c_char = ptr::null_mut();
        let pass = self.pass(slot.tech, slot.pass);
        // SAFETY: the built text (a C string or NULL) and the location, a C
        // string, outlive the call; the pass is valid.
        unsafe {
            let shader = match slot.kind {
                ShaderType::Vertex => {
                    let s = gs_vertexshader_create(
                        built.shader_str.array,
                        location.as_ptr().cast(),
                        &mut errors,
                    );
                    (*pass).vertshader = s;
                    s
                }
                ShaderType::Pixel => {
                    let s = gs_pixelshader_create(
                        built.shader_str.array,
                        location.as_ptr().cast(),
                        &mut errors,
                    );
                    (*pass).pixelshader = s;
                    s
                }
            };
            (NonNull::new(shader), errors)
        }
    }
    fn errors_nonempty(&self, errors: &*mut c_char) -> bool {
        // SAFETY: NULL or the C string the device handed back.
        !errors.is_null() && unsafe { **errors } != 0
    }
    fn add_shader_error(&mut self, errors: &*mut c_char) {
        // SAFETY: the parser, and a C string that outlives the call.
        unsafe {
            cf_adderror(
                &raw mut (*self.ep).cfp,
                c"Error creating shader: $1".as_ptr(),
                LEX_ERROR,
                *errors,
                ptr::null(),
                ptr::null(),
            );
        }
    }
    fn free_errors(&mut self, errors: *mut c_char) {
        // SAFETY: NULL or the device's bmalloc'd string.
        unsafe { bfree(errors.cast()) };
    }
    fn used_count(&self, built: &CBuilt) -> usize {
        built.used_params.num
    }
    fn resize_shader_params(&mut self, slot: ShaderSlot, num: usize) {
        // SAFETY: the pass's pass_shaderparam_array_t.
        unsafe {
            da_resize(
                &mut *self.shader_params(slot),
                size_of::<pass_shaderparam>(),
                num,
            );
        }
    }
    fn shader_param_count(&self, slot: ShaderSlot) -> usize {
        // SAFETY: as above.
        unsafe { (*self.shader_params(slot)).num }
    }
    fn effect_param(&mut self, built: &CBuilt, i: usize) -> *mut gs_effect_param {
        // SAFETY: the effect and a used param's name, a C string.
        unsafe { gs_effect_get_param_by_name(self.effect, Self::used_name(built, i)) }
    }
    fn shader_param(
        &mut self,
        shader: NonNull<c_void>,
        built: &CBuilt,
        i: usize,
    ) -> Option<NonNull<c_void>> {
        // SAFETY: a shader the device created and a used param's name.
        NonNull::new(unsafe {
            gs_shader_get_param_by_name(shader.as_ptr(), Self::used_name(built, i))
        })
    }
    fn set_shader_param(
        &mut self,
        slot: ShaderSlot,
        i: usize,
        eparam: *mut gs_effect_param,
        sparam: Option<NonNull<c_void>>,
    ) {
        // SAFETY: i is below the size the core gave the array.
        unsafe {
            let param: *mut pass_shaderparam = Self::at(&*self.shader_params(slot), i);
            (*param).eparam = eparam;
            (*param).sparam = sparam.map_or(ptr::null_mut(), NonNull::as_ptr);
        }
    }
    fn free_built(&mut self, built: CBuilt) {
        // SAFETY: dstr_array_free, da_free and dstr_free of the locals,
        // all on libobs's allocator.
        unsafe {
            for i in 0..built.used_params.num {
                bfree((*Self::at::<dstr>(&built.used_params, i)).array.cast());
            }
            bfree(built.used_params.array);
            bfree(built.shader_str.array.cast());
        }
    }
    fn log(&mut self, line: CompileLog<'_>) {
        let (which, idx, name) = match line {
            CompileLog::ShaderParamNotFound => (0, 0, &[][..]),
            CompileLog::MissingVertexShader { idx, name } => (1, idx, name),
            CompileLog::MissingPixelShader { idx, name } => (2, idx, name),
        };
        let mut name = name.to_vec();
        name.push(0);
        // SAFETY: a C string that outlives the call.
        unsafe { obs_graphics_effect_compile_error(which, idx, name.as_ptr().cast()) };
    }
}

/// `bool ep_compile(struct effect_parser *ep)`.
///
/// `ep_parse()` calls it once the text parsed with no error. It fills
/// `ep->effect` in place as the C does: its params, annotations,
/// techniques, passes and pass shader params grown as `da_resize` grows
/// them (new items zero), each name a `bstrdup` copy and each default value
/// moved out of the parser as `da_move` moves it, all on libobs's
/// allocator; each parser param points at its effect param. The shaders
/// come from `gs_vertexshader_create()` and `gs_pixelshader_create()` on
/// the text the writer builds, the shader params from
/// `gs_shader_get_param_by_name()` and `gs_effect_get_param_by_name()`, a
/// shader's error text goes to `cf_adderror()`, and the two missing-shader
/// lines and the missing shader param line go to `blog()`. Whatever a
/// failed pass created stays in the effect for `effect_free()`.
///
/// A null `ep`, or a parser with no effect, returns false (the C asserts
/// and dereferences it; `ep_parse()` never passes one).
///
/// # Safety
///
/// `ep` is null or a valid parser as `ep_parse()` leaves it, with
/// `ep->effect` null or a valid `struct gs_effect` it compiles into, on
/// the graphics thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ep_compile(ep: *mut effect_parser) -> bool {
    if ep.is_null() {
        return false;
    }
    // SAFETY: the caller's contract.
    let effect = unsafe { (*ep).effect }.cast::<gs_effect>();
    if effect.is_null() {
        return false;
    }
    compile::compile(&mut CCompile { ep, effect })
}
