//! `oracle_*` build of `libobs/graphics/effect-parser-compile.c.h`
//! (oracle/effect_parser_compile.c): the effect parser's compile step,
//! renamed, on the C writer, param types, effect param lookup and
//! `cf_adderror`, with helpers that set up and free a parser and its
//! effect, and the recording graphics stand-in it and the Rust shim call
//! (oracle/effect_parser_compile_device.c).

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    /// `bool ep_compile(struct effect_parser *)`.
    pub fn oracle_ep_compile(ep: *mut c_void) -> bool;
    /// `ep_init`, `effect_init` and `ep->effect = effect`, then
    /// `cf_parser_parse` of a short text from `file` (NULL allowed), with
    /// `cur_token` at its end token; false when the parse failed.
    pub fn oracle_effect_parser_compile_prepare(
        ep: *mut c_void,
        effect: *mut c_void,
        file: *const c_char,
    ) -> bool;
    /// Frees the parser's params and techniques, its arrays and its cf
    /// parser as `ep_free()` does, then `effect_free(effect)`.
    pub fn oracle_effect_parser_compile_free(ep: *mut c_void, effect: *mut c_void);
    /// `effect_free(effect)`, then `effect_init(effect)`.
    pub fn oracle_effect_parser_compile_effect_reset(effect: *mut c_void);
    /// Size, alignment and field offsets of `struct pass_shaderparam`,
    /// `ep_param` and `effect_parser` (with the size of
    /// `enum effect_section`), written to `out`; returns how many.
    pub fn oracle_effect_parser_compile_layout(out: *mut usize) -> usize;

    /// Clears the stand-in's event log and starts recording on this
    /// thread: create call `n` returns NULL for bit `n` of `null_mask`,
    /// hands back `"fail <n>"` for bit `n` of `error_mask` or `""` for bit
    /// `n` of `empty_mask`; the shaders of the calls in `missing_mask` lack
    /// the param `missing` (a C string that outlives the recording).
    pub fn oracle_compile_device_reset(
        null_mask: u32,
        error_mask: u32,
        empty_mask: u32,
        missing_mask: u32,
        missing: *const c_char,
    );
    /// Stops recording on this thread.
    pub fn oracle_compile_device_stop();
    /// The event log, `*len` bytes: each create and shader param call with
    /// what it answered, and each line the log handler got.
    pub fn oracle_compile_device_events(len: *mut usize) -> *const c_char;
    /// The log handler that adds the recording thread's `blog()` lines to
    /// the event log.
    pub fn oracle_compile_log_handler_ptr() -> *mut c_void;
}
