//! `oracle_*` build of `libobs/obs-output-caption-text.c.h`
//! (oracle/output_caption_text.c): the exported page function renamed, and
//! wrappers around the file's static functions.

use core::ffi::c_char;

/// Index of each value in `oracle_output_caption_text_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 4] = [
    "CAPTION_LINE_CHARS",
    "CAPTION_PAGE_ROWS",
    "CAPTION_LINE_BYTES",
    "CAPTION_PAGE_BYTES",
];

unsafe extern "C" {
    /// `obs_output_caption_text_page`: `page` has `CAPTION_PAGE_BYTES + 1`
    /// bytes.
    pub fn oracle_obs_output_caption_text_page(
        text: *mut *const c_char,
        page: *mut c_char,
    ) -> usize;
    /// `caption_text_row`: `row` has `CAPTION_LINE_BYTES + 1` bytes.
    pub fn oracle_output_caption_text_row(text: *mut *const c_char, row: *mut c_char) -> usize;
    pub fn oracle_output_caption_text_utf8_continuation(c: u8) -> bool;
    pub fn oracle_output_caption_text_utf8_char_length(text: *const c_char) -> usize;
    pub fn oracle_output_caption_text_is_newline(text: *const c_char) -> bool;
    pub fn oracle_output_caption_text_utf8_whitespace(text: *const c_char, bytes: usize) -> bool;
    /// The value named `VALUE_NAMES[which]`, 0 past the end.
    pub fn oracle_output_caption_text_value(which: usize) -> usize;
}
