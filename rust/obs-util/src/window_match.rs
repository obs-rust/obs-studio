//! Safe core of `libobs/util/windows/window-match.c.h`: the pure decisions
//! of the window helpers (`window-helpers.c`).
//!
//! They decide which windows the window and game capture property lists
//! show, and which window a capture source picks when it looks for its saved
//! window again:
//!
//! - [`is_internal_exe`]: Microsoft shell and system processes the list
//!   hides ([`INTERNAL_EXES_EXACT`] and the prefix in
//!   [`INTERNAL_EXES_PARTIAL`], without regard to ASCII case);
//! - [`is_untitled_explorer`]: `explorer.exe` (this exact case) with an
//!   empty title, which the list hides too;
//! - [`is_generic_class`]: a saved class containing one of
//!   [`GENERIC_CLASS_SUBSTRINGS`] (without regard to ASCII case) says little
//!   about the window, so the class priority falls back to the title;
//! - [`is_uwp_class`]: the saved class of a UWP or WinUI 3 window
//!   ([`UWP_CLASSES`]), which matches on its title alone;
//! - [`rating`]: how well a window matches the saved one. 0 is exact, lower
//!   is better, [`NO_MATCH`] is no match.
//!
//! `window-helpers.c` keeps every Win32 call, the reads of each window's
//! exe, title and class where they were, the three `dstr_cmpi()` compares
//! (their results arrive here), the window walk, the best-so-far compares
//! and the list building. These keep no state, take no lock and call
//! nothing in Win32.
//!
//! Case folding is [`crate::dstr`]'s `astrcmpi`, `astrcmpi_n` and
//! `astrstri`, which fold ASCII only, as C's `tolower()` does in the C
//! locale libobs runs in. Where the C is undefined (`abs(INT_MIN)`, and a
//! title distance so large that the penalty passes `INT_MAX`; `astrcmpi()`
//! never returns either), [`rating`] wraps. Nothing here can panic on any
//! input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::dstr::{astrcmpi, astrcmpi_n, astrstri};

/// `MS_WINDOW_MATCH_PRIORITY_CLASS` (`WINDOW_PRIORITY_CLASS`): match the
/// class, then the title, and prefer the same exe.
pub const PRIORITY_CLASS: i32 = 0;
/// `MS_WINDOW_MATCH_PRIORITY_TITLE` (`WINDOW_PRIORITY_TITLE`): match the
/// exact title.
pub const PRIORITY_TITLE: i32 = 1;
/// `MS_WINDOW_MATCH_PRIORITY_EXE` (`WINDOW_PRIORITY_EXE`): match the exe,
/// then the title.
pub const PRIORITY_EXE: i32 = 2;

/// `MS_WINDOW_MATCH_NONE`: the rating of a window that does not match.
pub const NO_MATCH: i32 = 0x7FFF_FFFF;

/// `MS_WINDOW_MATCH_EXE_PENALTY`: added under the class priority when the
/// class matches but the exe does not.
pub const EXE_PENALTY: i32 = 0x1000;

/// `internal_microsoft_exes_exact`: not capturable or internal windows,
/// exact executable names.
pub const INTERNAL_EXES_EXACT: [&[u8]; 15] = [
    b"startmenuexperiencehost.exe",
    b"applicationframehost.exe",
    b"peopleexperiencehost.exe",
    b"shellexperiencehost.exe",
    b"microsoft.notes.exe",
    b"systemsettings.exe",
    b"textinputhost.exe",
    b"searchapp.exe",
    b"video.ui.exe",
    b"searchui.exe",
    b"lockapp.exe",
    b"cortana.exe",
    b"gamebar.exe",
    b"tabtip.exe",
    b"time.exe",
];

/// `internal_microsoft_exes_partial`: partial matches start from the
/// beginning of the executable name.
pub const INTERNAL_EXES_PARTIAL: [&[u8]; 1] = [b"windowsinternal"];

/// `generic_class_substrings`.
pub const GENERIC_CLASS_SUBSTRINGS: [&[u8]; 2] = [b"Chrome", b"SDL_app"];

/// The two classes `ms_window_match_is_uwp_class()` compares with.
pub const UWP_CLASSES: [&[u8]; 2] = [
    b"Windows.UI.Core.CoreWindow",
    b"WinUIDesktopWin32WindowClass",
];

/// The exe `ms_window_match_is_untitled_explorer()` compares with.
pub const EXPLORER_EXE: &[u8] = b"explorer.exe";

/// `ms_window_match_is_internal_exe`: whether the list hides windows of
/// this exe. `None` (NULL) is not internal.
pub fn is_internal_exe(exe: Option<&[u8]>) -> bool {
    let Some(exe) = exe else {
        return false;
    };
    INTERNAL_EXES_EXACT
        .iter()
        .any(|name| astrcmpi(Some(exe), Some(name)) == 0)
        || INTERNAL_EXES_PARTIAL
            .iter()
            .any(|prefix| astrcmpi_n(Some(exe), Some(prefix), prefix.len()) == 0)
}

/// `ms_window_match_is_untitled_explorer`: whether the list hides a window
/// of this exe because it is `explorer.exe`, byte for byte, with an empty
/// title. `None` (NULL) compares as the empty string.
pub fn is_untitled_explorer(exe: Option<&[u8]>, title_empty: bool) -> bool {
    cstr(exe.unwrap_or_default()) == EXPLORER_EXE && title_empty
}

/// `ms_window_match_is_generic_class`: whether the saved class contains
/// one of [`GENERIC_CLASS_SUBSTRINGS`], without regard to case. `None`
/// (NULL) contains nothing.
pub fn is_generic_class(window_class: Option<&[u8]>) -> bool {
    GENERIC_CLASS_SUBSTRINGS
        .iter()
        .any(|find| astrstri(window_class, Some(find)).is_some())
}

/// `ms_window_match_is_uwp_class`: whether the saved class is one of
/// [`UWP_CLASSES`], byte for byte.
pub fn is_uwp_class(window_class: &[u8]) -> bool {
    let window_class = cstr(window_class);
    UWP_CLASSES.contains(&window_class)
}

/// `ms_window_match_rating`: how well a window matches the saved one.
///
/// `priority` is the source's `enum window_priority`; `uwp_window` and
/// `generic_class` are [`is_uwp_class`] and [`is_generic_class`] of the
/// saved class; `class_matches` and `exe_matches` say whether the window's
/// class and exe equal the saved ones without regard to case; `title_cmp`
/// is `dstr_cmpi()` of the window's title with the saved one, whose
/// absolute value is the title's distance.
pub fn rating(
    priority: i32,
    uwp_window: bool,
    generic_class: bool,
    class_matches: bool,
    exe_matches: bool,
    title_cmp: i32,
) -> i32 {
    let title_val = title_cmp.wrapping_abs();
    let priority = if generic_class && priority == PRIORITY_CLASS {
        PRIORITY_TITLE
    } else {
        priority
    };

    // always match by name with UWP windows
    if uwp_window {
        if priority == PRIORITY_EXE && !exe_matches {
            NO_MATCH
        } else {
            exact(title_val)
        }
    } else if priority == PRIORITY_CLASS {
        let val = if class_matches { title_val } else { NO_MATCH };
        if val != NO_MATCH && !exe_matches {
            val.wrapping_add(EXE_PENALTY)
        } else {
            val
        }
    } else if priority == PRIORITY_TITLE {
        exact(title_val)
    } else if priority == PRIORITY_EXE {
        if exe_matches { title_val } else { NO_MATCH }
    } else {
        NO_MATCH
    }
}

/// 0 for an exact title (distance 0), else [`NO_MATCH`].
fn exact(title_val: i32) -> i32 {
    if title_val == 0 { 0 } else { NO_MATCH }
}

/// The bytes of a C string: up to the first NUL, as `strcmp()` reads them.
fn cstr(s: &[u8]) -> &[u8] {
    s.iter()
        .position(|&c| c == 0)
        .and_then(|n| s.get(..n))
        .unwrap_or(s)
}

/// The escape of one byte of a saved window part: `#` as `#22`, `:` as
/// `#3A`, any other byte as itself.
fn escaped(c: &u8) -> &[u8] {
    match c {
        b'#' => b"#22",
        b':' => b"#3A",
        _ => core::slice::from_ref(c),
    }
}

/// `ms_window_match_encode` without its buffer: `s` (up to its first NUL)
/// with `#` escaped as `#22` and `:` as `#3A`, the form each part of a
/// saved `title:class:exe` string takes.
pub fn encode(s: &[u8]) -> Vec<u8> {
    cstr(s).iter().flat_map(escaped).copied().collect()
}

/// The length of [`encode`] of `s`, without building it.
pub fn encoded_len(s: &[u8]) -> usize {
    cstr(s).iter().map(|c| escaped(c).len()).sum()
}

/// `ms_window_match_encode`: [`encode`] of `s` into `out` like
/// `snprintf()`. It returns the full escaped length and writes at most
/// `out.len() - 1` bytes of it, then a NUL; an empty `out` is left alone.
pub fn encode_into(s: &[u8], out: &mut [u8]) -> usize {
    if let Some(room) = out.len().checked_sub(1) {
        let bytes = cstr(s).iter().flat_map(escaped);
        let written = out
            .iter_mut()
            .take(room)
            .zip(bytes)
            .map(|(dst, src)| *dst = *src)
            .count();
        if let Some(end) = out.get_mut(written) {
            *end = 0;
        }
    }
    encoded_len(s)
}

/// `ms_window_match_decode`: `s` (up to its first NUL) decoded in place,
/// every `#3A` (this case) to `:`, then every `#22` to `#`, each pass left
/// to right and never into what it wrote (`#2222` is `#22`). Each pass
/// writes a NUL after its result when that is inside `s`, and the bytes
/// past it are left as the pass left them. Returns the new length.
pub fn decode(s: &mut [u8]) -> usize {
    let len = cstr(s).len();
    let len = decode_pass(s, len, *b"3A", b':');
    decode_pass(s, len, *b"22", b'#')
}

/// One `dstr_replace()` of `#` and `code` with `to` over the first `len`
/// bytes of `s`: the leftmost match first, the scan going on after it.
fn decode_pass(s: &mut [u8], len: usize, code: [u8; 2], to: u8) -> usize {
    let mut read = 0usize;
    let mut write = 0usize;
    loop {
        let (byte, step) = match s.get(read..len).unwrap_or_default() {
            [b'#', a, b, ..] if [*a, *b] == code => (to, 3),
            [c, ..] => (*c, 1),
            [] => break,
        };
        if let Some(dst) = s.get_mut(write) {
            *dst = byte;
        }
        write = write.saturating_add(1);
        read = read.saturating_add(step);
    }
    if let Some(end) = s.get_mut(write) {
        *end = 0;
    }
    write
}

/// `ms_window_match_split`: the first three fields of `s` (up to its
/// first NUL) split at `:`, empty ones kept, the third ending at the next
/// `:` or the end, later fields ignored. `None` with fewer than three
/// fields.
pub fn split(s: &[u8]) -> Option<[core::ops::Range<usize>; 3]> {
    let mut fields = cstr(s).split(|&c| c == b':').scan(0usize, |start, field| {
        let range = *start..start.saturating_add(field.len());
        *start = range.end.saturating_add(1);
        Some(range)
    });
    Some([fields.next()?, fields.next()?, fields.next()?])
}
