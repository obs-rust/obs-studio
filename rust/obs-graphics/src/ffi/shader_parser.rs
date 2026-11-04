//! C ABI for `libobs/graphics/shader-parser-blocks.c.h`: the shader
//! parser's sampler-state and struct blocks.
//!
//! `shader_parse()` (`shader-parser.c`) calls [`sp_parse_sampler_state`] and
//! [`sp_parse_struct`] on the keyword token that starts each block. They are
//! declared in `graphics/shader-parser-blocks.h` without `EXPORT`, so the
//! shims stay hidden in libobs (`libobs/cmake/rust-hidden.d`).
//!
//! The core walks `sp->cfp.cur_token` in place ([`RawTokens`]), so
//! `shader_parse()` goes on from the token where the block stopped, as
//! before. Its errors go to the exported `cf_adderror()` and its `{ ... }`
//! skips to the exported `cf_pass_pair()`, the calls the C makes, each at
//! the same token. A block that parses is written into `sp->samplers` or
//! `sp->structs` as the C builds it: every string a `bstrdup_n()` copy and
//! every array a darray, grown as `util/darray.h` grows it, all on libobs's
//! allocator, so `shader_parser_free()`'s inline `shader_sampler_free()` and
//! `shader_struct_free()` free them. A struct variable is `shader_var_init()`
//! (all zero) with its type, name and mapping.
//!
//! libobs's allocator and the two `cf-parser.c` exports are reached through
//! the private C adapters in `libobs/graphics/obs-graphics-adapter.c.h`,
//! not obs-util's Rust paths: see the note in `ffi/image_file.rs`.
//!
//! A null `sp` does nothing (`shader_parse()` never passes one; the C
//! dereferences it).

use core::ffi::{CStr, c_char, c_int, c_void};
use core::mem::size_of;
use core::ptr;

use obs_util::ffi::cf_parser::cf_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;

use crate::ffi::shader_sampler_convert::shader_sampler;
use crate::shader_parser::{self as core_blocks, BlockError, Sampler, Struct, Tokens};

/// `struct shader_var` (`graphics/shader-parser.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct shader_var {
    pub r#type: *mut c_char,
    pub name: *mut c_char,
    pub mapping: *mut c_char,
    pub var_type: c_int,
    pub array_count: c_int,
    pub gl_sampler_id: usize,
    pub default_val: darray,
}

/// `struct shader_struct`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct shader_struct {
    pub name: *mut c_char,
    pub vars: darray,
}

/// `struct shader_parser`. Only `structs` and `samplers` are written here.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct shader_parser {
    pub cfp: cf_parser,
    pub params: darray,
    pub structs: darray,
    pub samplers: darray,
    pub funcs: darray,
}

unsafe extern "C" {
    // libobs's allocator and cf-parser exports, through the private C
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
    #[link_name = "obs_graphics_cf_pass_pair"]
    fn cf_pass_pair(p: *mut cf_parser, open: c_char, close: c_char) -> bool;
}

/// `sp->cfp` as the core's [`Tokens`]: `cur_token` itself.
pub(crate) struct RawTokens(pub(crate) *mut cf_parser);

impl RawTokens {
    fn token(&self) -> &cf_token {
        // SAFETY: the parser's cur_token points into its terminated token
        // array (the shim's contract), and the core never moves it past the
        // terminator.
        unsafe { &*(*self.0).cur_token }
    }
}

impl Tokens for RawTokens {
    type Mark = *mut cf_token;

    fn kind(&self) -> i32 {
        self.token().r#type
    }

    fn text(&self) -> &[u8] {
        let s = &self.token().str;
        if s.array.is_null() {
            return &[];
        }
        // SAFETY: a token's str is len readable bytes of the lexer's text.
        unsafe { core::slice::from_raw_parts(s.array.cast::<u8>(), s.len) }
    }

    fn advance(&mut self) {
        // SAFETY: the core advances only from a token that is not the
        // terminator, so the next one is in the array.
        unsafe { (*self.0).cur_token = (*self.0).cur_token.add(1) };
    }

    fn mark(&self) -> Self::Mark {
        // SAFETY: the parser is valid.
        unsafe { (*self.0).cur_token }
    }

    fn reset(&mut self, mark: Self::Mark) {
        // SAFETY: mark is a cur_token this parser held.
        unsafe { (*self.0).cur_token = mark };
    }

    fn add_error(&mut self, error: BlockError) {
        let (message, val1) = error.message();
        // SAFETY: a valid parser at a valid token; the strings are static C
        // strings, as the C passes them, at the C's level.
        unsafe {
            cf_adderror(
                self.0,
                message.as_ptr(),
                error.level(),
                val1.map_or(ptr::null(), CStr::as_ptr),
                ptr::null(),
                ptr::null(),
            );
        }
    }

    // c_char is i8 on most targets and u8 on some (aarch64 Linux,
    // aarch64-pc-windows-msvc): the casts are needed on the first and
    // no-ops on the second, where clippy calls them unnecessary.
    #[allow(clippy::unnecessary_cast)]
    fn pass_pair(&mut self) -> bool {
        // SAFETY: a valid parser over a terminated token stream.
        unsafe { cf_pass_pair(self.0, b'{' as c_char, b'}' as c_char) }
    }
}

/// `bstrdup_n(str, n)`: a NUL-terminated `bmalloc` copy.
pub(crate) fn dup(bytes: &[u8]) -> *mut c_char {
    // SAFETY: bmalloc never returns null (it aborts); the copy fits.
    unsafe {
        let p = bmalloc(bytes.len().saturating_add(1)).cast::<u8>();
        ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        *p.add(bytes.len()) = 0;
        p.cast()
    }
}

/// `da_push_back(da, &value)`: `darray_ensure_capacity` (the capacity
/// doubles, or becomes `num + 1` from 0, the old items moved to a new
/// `bmalloc` buffer), then the item at the end.
///
/// # Safety
///
/// `da` is a valid darray of `T` (or empty), allocated with `bmalloc`.
pub(crate) unsafe fn push<T>(da: &mut darray, value: T) {
    let size = size_of::<T>();
    let new_size = da.num.saturating_add(1);
    if new_size > da.capacity {
        let new_cap = if da.capacity == 0 {
            new_size
        } else {
            da.capacity.saturating_mul(2).max(new_size)
        };
        // SAFETY: bmalloc never returns null; the old buffer holds
        // `capacity` items and came from bmalloc.
        unsafe {
            let p = bmalloc(size.saturating_mul(new_cap));
            if !da.array.is_null() {
                if da.capacity != 0 {
                    ptr::copy_nonoverlapping(
                        da.array.cast::<u8>(),
                        p.cast::<u8>(),
                        size.saturating_mul(da.capacity),
                    );
                }
                bfree(da.array);
            }
            da.array = p;
        }
        da.capacity = new_cap;
    }
    // SAFETY: the buffer holds more than `num` items of `T`.
    unsafe { ptr::write_unaligned(da.array.cast::<T>().add(da.num), value) };
    da.num = new_size;
}

pub(crate) const fn empty_darray() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

/// The C `struct shader_sampler` the C pushes for `ss`.
fn sampler_to_c(ss: &Sampler) -> shader_sampler {
    let mut out = shader_sampler {
        name: dup(&ss.name),
        states: empty_darray(),
        values: empty_darray(),
    };
    for (state, value) in ss.states.iter().zip(&ss.values) {
        // SAFETY: both start empty and are grown only here.
        unsafe {
            push(&mut out.states, dup(state));
            push(&mut out.values, dup(value));
        }
    }
    out
}

/// The C `struct shader_struct` the C pushes for `ss`.
fn struct_to_c(ss: &Struct) -> shader_struct {
    let mut out = shader_struct {
        name: dup(&ss.name),
        vars: empty_darray(),
    };
    for var in &ss.vars {
        let c_var = shader_var {
            r#type: dup(&var.type_name),
            name: dup(&var.name),
            mapping: var.mapping.as_deref().map_or(ptr::null_mut(), dup),
            var_type: 0,
            array_count: 0,
            gl_sampler_id: 0,
            default_val: empty_darray(),
        };
        // SAFETY: vars starts empty and is grown only here.
        unsafe { push(&mut out.vars, c_var) };
    }
    out
}

/// `void sp_parse_sampler_state(struct shader_parser *sp)`.
///
/// # Safety
///
/// `sp` is null or an initialized `shader_parser` whose `cfp.cur_token`
/// points into its terminated token array; `sp->samplers` is a valid darray
/// of `struct shader_sampler`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sp_parse_sampler_state(sp: *mut shader_parser) {
    if sp.is_null() {
        return;
    }
    // SAFETY: the caller's contract; cfp is the first field of sp.
    let mut tokens = RawTokens(unsafe { &raw mut (*sp).cfp });
    if let Some(ss) = core_blocks::parse_sampler_state(&mut tokens) {
        let c_ss = sampler_to_c(&ss);
        // SAFETY: samplers is a valid darray of struct shader_sampler.
        unsafe { push(&mut (*sp).samplers, c_ss) };
    }
}

/// `void sp_parse_struct(struct shader_parser *sp)`.
///
/// # Safety
///
/// As for [`sp_parse_sampler_state`], with `sp->structs` a valid darray of
/// `struct shader_struct`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sp_parse_struct(sp: *mut shader_parser) {
    if sp.is_null() {
        return;
    }
    // SAFETY: the caller's contract; cfp is the first field of sp.
    let mut tokens = RawTokens(unsafe { &raw mut (*sp).cfp });
    if let Some(ss) = core_blocks::parse_struct(&mut tokens) {
        let c_ss = struct_to_c(&ss);
        // SAFETY: structs is a valid darray of struct shader_struct.
        unsafe { push(&mut (*sp).structs, c_ss) };
    }
}
