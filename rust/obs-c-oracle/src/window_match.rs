//! `oracle_*` build of `libobs/util/windows/window-match.c.h`
//! (oracle/window_match.c), its compares on the oracle's C dstr.

use core::ffi::{c_char, c_int};

/// Index of each value in `oracle_window_match_value`, in the C's order.
pub const VALUE_NAMES: [&str; 5] = [
    "MS_WINDOW_MATCH_PRIORITY_CLASS",
    "MS_WINDOW_MATCH_PRIORITY_TITLE",
    "MS_WINDOW_MATCH_PRIORITY_EXE",
    "MS_WINDOW_MATCH_NONE",
    "MS_WINDOW_MATCH_EXE_PENALTY",
];

unsafe extern "C" {
    pub fn oracle_ms_window_match_is_internal_exe(exe: *const c_char) -> bool;
    pub fn oracle_ms_window_match_is_untitled_explorer(
        exe: *const c_char,
        title_empty: bool,
    ) -> bool;
    pub fn oracle_ms_window_match_is_generic_class(window_class: *const c_char) -> bool;
    pub fn oracle_ms_window_match_is_uwp_class(window_class: *const c_char) -> bool;
    pub fn oracle_ms_window_match_rating(
        priority: c_int,
        uwp_window: bool,
        generic_class: bool,
        class_matches: bool,
        exe_matches: bool,
        title_cmp: c_int,
    ) -> c_int;
    pub fn oracle_ms_window_match_encode(
        s: *const c_char,
        out: *mut c_char,
        out_size: usize,
    ) -> usize;
    pub fn oracle_ms_window_match_decode(s: *mut c_char) -> usize;
    pub fn oracle_ms_window_match_split(
        str: *const c_char,
        start: *mut usize,
        len: *mut usize,
    ) -> bool;
    /// The value at `which` in [`VALUE_NAMES`], or -1 past the end.
    pub fn oracle_window_match_value(which: usize) -> c_int;
}
