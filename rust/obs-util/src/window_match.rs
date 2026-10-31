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
//! locale libobs runs in. Nothing here can panic on any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

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
    let _ = exe;
    todo!()
}

/// `ms_window_match_is_untitled_explorer`: whether the list hides a window
/// of this exe because it is `explorer.exe`, byte for byte, with an empty
/// title. `None` (NULL) compares as the empty string.
pub fn is_untitled_explorer(exe: Option<&[u8]>, title_empty: bool) -> bool {
    let _ = (exe, title_empty);
    todo!()
}

/// `ms_window_match_is_generic_class`: whether the saved class contains
/// one of [`GENERIC_CLASS_SUBSTRINGS`], without regard to case. `None`
/// (NULL) contains nothing.
pub fn is_generic_class(window_class: Option<&[u8]>) -> bool {
    let _ = window_class;
    todo!()
}

/// `ms_window_match_is_uwp_class`: whether the saved class is one of
/// [`UWP_CLASSES`], byte for byte.
pub fn is_uwp_class(window_class: &[u8]) -> bool {
    let _ = window_class;
    todo!()
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
    let _ = (
        priority,
        uwp_window,
        generic_class,
        class_matches,
        exe_matches,
        title_cmp,
    );
    todo!()
}
