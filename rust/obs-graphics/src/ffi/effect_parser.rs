//! C ABI for `libobs/graphics/effect-parser-blocks.c.h`: the effect
//! parser's struct and sampler-state blocks.
//!
//! `ep_parse()` (`effect-parser.c`) calls [`ep_parse_struct`] and
//! [`ep_parse_sampler_state`] on the keyword token that starts each block.
//! They are declared in `graphics/effect-parser-blocks.h` without `EXPORT`,
//! so the shims stay hidden in libobs (`libobs/cmake/rust-hidden.d`).
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
//! A null `ep` does nothing (`ep_parse()` never passes one; the C
//! dereferences it).

use core::ffi::{c_char, c_int};
use core::ptr;

use obs_util::ffi::darray::darray;

use crate::effect_parser::{self as core_blocks, Sampler, Struct};
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
