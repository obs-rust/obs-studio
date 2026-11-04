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
//! A null `ep` does nothing (`ep_parse()` never passes one; the C
//! dereferences it).

use core::ffi::{c_char, c_int, c_void};
use core::ptr;

use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;

use crate::effect_parser::{self as core_blocks, Pass, ProgramToken, Sampler, Struct, Technique};
use crate::ffi::effect_parser_lifecycle::effect_parser;
use crate::ffi::shader_parser::{RawTokens, dup, empty_darray, push};

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
