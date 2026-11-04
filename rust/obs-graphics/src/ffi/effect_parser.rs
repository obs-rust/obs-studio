//! C ABI for `libobs/graphics/effect-parser-blocks.c.h` and
//! `libobs/graphics/effect-parser-passes.c.h`: the effect parser's struct
//! and sampler-state blocks, and its passes and techniques.
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
//! A null `ep` does nothing (`ep_parse()` never passes one; the C
//! dereferences it).

use core::ffi::{CStr, c_char, c_int, c_void};
use core::mem::size_of;
use core::ptr;

use obs_util::ffi::cf_parser::cf_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;

use crate::effect_param_types::{self, Route};
use crate::effect_parser::{
    self as core_blocks, INVALID_TYPE, Param, ParamTokens, Pass, ProgramToken, Sampler, Struct,
    Technique,
};
use crate::ffi::effect_parser_lifecycle::effect_parser;
use crate::ffi::shader_param_values::libobs_strtod;
use crate::ffi::shader_parser::{RawTokens, dup, empty_darray, push};
use crate::shader_parser::LEX_ERROR;

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
