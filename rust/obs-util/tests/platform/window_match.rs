//! Tier 1: `obs_util::window_match`, the window helpers' pure decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_window_match_steps.c` 1:1:
//! its 15 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `ms_window_match_*` functions; here the
//! safe core is called directly.
//!
//! The `edge_*` cases are new: each decision against a model of the
//! origin/master C (its expressions inline, with ASCII folding written out)
//! over built inputs, and proptests over arbitrary bytes and values.

use obs_util::window_match::{
    EXE_PENALTY, EXPLORER_EXE, GENERIC_CLASS_SUBSTRINGS, INTERNAL_EXES_EXACT,
    INTERNAL_EXES_PARTIAL, NO_MATCH, PRIORITY_CLASS, PRIORITY_EXE, PRIORITY_TITLE, UWP_CLASSES,
    is_generic_class, is_internal_exe, is_untitled_explorer, is_uwp_class, rating,
};
use proptest::prelude::*;

const NONE: i32 = NO_MATCH;
const PENALTY: i32 = EXE_PENALTY;

/// `s` in a case pattern: 0 as is, 1 upper, 2 first upper, 3 alternate.
fn recase(s: &[u8], pattern: u8) -> Vec<u8> {
    s.iter()
        .enumerate()
        .map(|(i, &c)| {
            let up = pattern == 1 || (pattern == 2 && i == 0) || (pattern == 3 && i % 2 == 1);
            if up { c.to_ascii_uppercase() } else { c }
        })
        .collect()
}

fn cat(a: &[u8], b: &[u8]) -> Vec<u8> {
    [a, b].concat()
}

fn internal(exe: &[u8]) -> bool {
    is_internal_exe(Some(exe))
}

fn rate(
    priority: i32,
    uwp: bool,
    generic: bool,
    class_matches: bool,
    exe_matches: bool,
    title_cmp: i32,
) -> i32 {
    rating(
        priority,
        uwp,
        generic,
        class_matches,
        exe_matches,
        title_cmp,
    )
}

#[test]
fn steps_constants() {
    assert_eq!(PRIORITY_CLASS, 0);
    assert_eq!(PRIORITY_TITLE, 1);
    assert_eq!(PRIORITY_EXE, 2);
    assert_eq!(NONE, 0x7FFF_FFFF);
    assert_eq!(PENALTY, 0x1000);
}

#[test]
fn steps_internal_exact_names() {
    for name in INTERNAL_EXES_EXACT {
        for pattern in 0..4 {
            assert!(
                internal(&recase(name, pattern)),
                "{:?} {pattern}",
                name.escape_ascii().to_string()
            );
        }
        assert!(!internal(&cat(name, b"x")));
        assert!(!internal(&cat(b"x", name)));
        assert!(!internal(&name[..name.len() - 1]));
        assert!(!internal(&cat(name, b" ")));
    }
}

#[test]
fn steps_internal_prefix() {
    assert!(internal(b"windowsinternal"));
    assert!(internal(b"windowsinternal.foo.exe"));
    assert!(internal(b"WindowsInternal"));
    assert!(internal(b"WINDOWSINTERNAL.CBSPREVIEW.EXE"));
    assert!(internal(b"windowsinternal\xc3\xa9"));
    assert!(!internal(b"windowsinterna"));
    assert!(!internal(b"xwindowsinternal"));
    assert!(!internal(b" windowsinternal"));
}

#[test]
fn steps_internal_other_exes() {
    assert!(!is_internal_exe(None));
    for exe in [
        &b""[..],
        b"explorer.exe",
        b"obs64.exe",
        b"chrome.exe",
        b"unknown",
        b"time",
        b".exe",
        b"\xc3\xa9",
    ] {
        assert!(!internal(exe), "{}", exe.escape_ascii());
    }
}

#[test]
fn steps_untitled_explorer() {
    assert!(is_untitled_explorer(Some(b"explorer.exe"), true));
    assert!(!is_untitled_explorer(Some(b"explorer.exe"), false));

    // characterized, not endorsed: Explorer.exe with an empty title is
    // listed
    assert!(!is_untitled_explorer(Some(b"Explorer.exe"), true));
    assert!(!is_untitled_explorer(Some(b"EXPLORER.EXE"), true));

    assert!(!is_untitled_explorer(Some(b"explorer.ex"), true));
    assert!(!is_untitled_explorer(Some(b"explorer.exe "), true));
    assert!(!is_untitled_explorer(Some(b"xexplorer.exe"), true));
    assert!(!is_untitled_explorer(Some(b""), true));
    assert!(!is_untitled_explorer(None, true));
    assert!(!is_untitled_explorer(None, false));
}

#[test]
fn steps_generic_class() {
    let generic: [&[u8]; 10] = [
        b"Chrome_WidgetWin_1",
        b"Chrome_RenderWidgetHostHWND",
        b"chrome",
        b"CHROME",
        b"xCHROMEx",
        b"MozillaChromeWindow",
        b"SDL_app",
        b"sdl_APP",
        b"MySDL_appWindow",
        b"\xc3\xa9Chrome",
    ];
    let specific: [&[u8]; 9] = [
        b"",
        b"Chrom",
        b"hrome",
        b"SDL_ap",
        b"DL_app",
        b"SDL app",
        b"Notepad",
        b"Windows.UI.Core.CoreWindow",
        b"\xc3\xa9",
    ];
    for class in generic {
        assert!(is_generic_class(Some(class)), "{}", class.escape_ascii());
    }
    for class in specific {
        assert!(!is_generic_class(Some(class)), "{}", class.escape_ascii());
    }
}

#[test]
fn steps_uwp_class() {
    let other: [&[u8]; 9] = [
        b"",
        b"windows.ui.core.corewindow",
        b"WINDOWS.UI.CORE.COREWINDOW",
        b"Windows.UI.Core.CoreWindo",
        b"Windows.UI.Core.CoreWindowx",
        b"winuidesktopwin32windowclass",
        b"WinUIDesktopWin32WindowClas",
        b"ApplicationFrameWindow",
        b"Chrome_WidgetWin_1",
    ];
    assert!(is_uwp_class(b"Windows.UI.Core.CoreWindow"));
    assert!(is_uwp_class(b"WinUIDesktopWin32WindowClass"));
    for class in other {
        assert!(!is_uwp_class(class), "{}", class.escape_ascii());
    }
}

#[test]
fn steps_rating_class_priority() {
    assert_eq!(rate(PRIORITY_CLASS, false, false, true, true, 0), 0);
    assert_eq!(rate(PRIORITY_CLASS, false, false, true, true, 1), 1);
    assert_eq!(rate(PRIORITY_CLASS, false, false, true, true, -1), 1);
    assert_eq!(rate(PRIORITY_CLASS, false, false, true, true, 200), 200);
    assert_eq!(rate(PRIORITY_CLASS, false, false, true, true, -200), 200);
    assert_eq!(rate(PRIORITY_CLASS, false, false, true, false, 0), PENALTY);
    assert_eq!(
        rate(PRIORITY_CLASS, false, false, true, false, -7),
        PENALTY + 7
    );
    assert_eq!(rate(PRIORITY_CLASS, false, false, false, true, 0), NONE);
    assert_eq!(rate(PRIORITY_CLASS, false, false, false, false, 0), NONE);
    assert_eq!(rate(PRIORITY_CLASS, false, false, false, false, 200), NONE);
}

#[test]
fn steps_rating_title_priority() {
    for m in 0..4 {
        let (class_matches, exe_matches) = (m & 1 != 0, m & 2 != 0);
        assert_eq!(
            rate(PRIORITY_TITLE, false, false, class_matches, exe_matches, 0),
            0
        );
        assert_eq!(
            rate(PRIORITY_TITLE, false, false, class_matches, exe_matches, 1),
            NONE
        );
        assert_eq!(
            rate(PRIORITY_TITLE, false, false, class_matches, exe_matches, -1),
            NONE
        );
        assert_eq!(
            rate(
                PRIORITY_TITLE,
                false,
                false,
                class_matches,
                exe_matches,
                200
            ),
            NONE
        );
    }
}

#[test]
fn steps_rating_exe_priority() {
    for class_matches in [false, true] {
        assert_eq!(rate(PRIORITY_EXE, false, false, class_matches, true, 0), 0);
        assert_eq!(rate(PRIORITY_EXE, false, false, class_matches, true, -1), 1);
        assert_eq!(
            rate(PRIORITY_EXE, false, false, class_matches, true, 200),
            200
        );
        assert_eq!(
            rate(PRIORITY_EXE, false, false, class_matches, false, 0),
            NONE
        );
        assert_eq!(
            rate(PRIORITY_EXE, false, false, class_matches, false, 1),
            NONE
        );
    }
}

#[test]
fn steps_rating_generic_class() {
    assert_eq!(rate(PRIORITY_CLASS, false, true, true, true, 0), 0);
    assert_eq!(rate(PRIORITY_CLASS, false, true, true, true, 1), NONE);
    assert_eq!(rate(PRIORITY_CLASS, false, true, true, false, 0), 0);
    assert_eq!(rate(PRIORITY_CLASS, false, true, false, false, 0), 0);
    assert_eq!(rate(PRIORITY_CLASS, false, true, false, false, 200), NONE);
    assert_eq!(rate(PRIORITY_TITLE, false, true, false, false, 0), 0);
    assert_eq!(rate(PRIORITY_TITLE, false, true, true, true, 1), NONE);
    assert_eq!(rate(PRIORITY_EXE, false, true, false, true, 5), 5);
    assert_eq!(rate(PRIORITY_EXE, false, true, true, false, 0), NONE);
}

#[test]
fn steps_rating_uwp() {
    for generic in [false, true] {
        for class_matches in [false, true] {
            for exe_matches in [false, true] {
                assert_eq!(
                    rate(PRIORITY_CLASS, true, generic, class_matches, exe_matches, 0),
                    0
                );
                assert_eq!(
                    rate(PRIORITY_CLASS, true, generic, class_matches, exe_matches, 1),
                    NONE
                );
                assert_eq!(
                    rate(PRIORITY_TITLE, true, generic, class_matches, exe_matches, 0),
                    0
                );
                assert_eq!(
                    rate(
                        PRIORITY_TITLE,
                        true,
                        generic,
                        class_matches,
                        exe_matches,
                        -1
                    ),
                    NONE
                );
                assert_eq!(
                    rate(PRIORITY_EXE, true, generic, class_matches, exe_matches, 0),
                    if exe_matches { 0 } else { NONE }
                );
                assert_eq!(
                    rate(PRIORITY_EXE, true, generic, class_matches, exe_matches, 200),
                    NONE
                );
            }
        }
    }
}

#[test]
fn steps_rating_unknown_priority() {
    // characterized, not endorsed: a priority outside the three matches
    // nothing, except a UWP window by its exact title
    for p in [3, -1, 100] {
        assert_eq!(rate(p, false, false, true, true, 0), NONE);
        assert_eq!(rate(p, false, true, true, true, 0), NONE);
        assert_eq!(rate(p, false, false, true, true, 5), NONE);
        assert_eq!(rate(p, true, false, false, false, 0), 0);
        assert_eq!(rate(p, true, false, true, true, 1), NONE);
    }
}

#[test]
fn steps_rating_penalty_bound() {
    assert_eq!(
        rate(PRIORITY_CLASS, false, false, true, false, 255),
        PENALTY + 255
    );
    assert_eq!(
        rate(PRIORITY_CLASS, false, false, true, false, -255),
        PENALTY + 255
    );
    assert!(rate(PRIORITY_CLASS, false, false, true, false, 255) < NONE);
    assert_eq!(rate(PRIORITY_EXE, false, false, false, true, -255), 255);
}

#[test]
fn steps_rating_grid() {
    for p in 0..=3 {
        for flags in 0..16 {
            let (uwp, generic, class_matches, exe_matches) = (
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
                flags & 8 != 0,
            );
            for cmp in [0, 1, -1, 200] {
                assert_eq!(
                    rate(p, uwp, generic, class_matches, exe_matches, cmp),
                    model_rating(p, uwp, generic, class_matches, exe_matches, cmp),
                    "rating({p}, {uwp}, {generic}, {class_matches}, {exe_matches}, {cmp})"
                );
            }
        }
    }
}

// ---- models of the origin/master C ----

/// `tolower()` in the C locale on an `unsigned char`.
fn fold(c: u8) -> u8 {
    if c.is_ascii_uppercase() { c | 0x20 } else { c }
}

/// The bytes of a C string: up to the first NUL.
fn cstr(s: &[u8]) -> &[u8] {
    s.iter().position(|&c| c == 0).map_or(s, |n| &s[..n])
}

/// `astrcmpi_n(a, b, n) == 0` (`n == usize::MAX` for `astrcmpi`).
fn model_eqi_n(a: &[u8], b: &[u8], n: usize) -> bool {
    let (a, b) = (cstr(a), cstr(b));
    let (a, b) = (&a[..a.len().min(n)], &b[..b.len().min(n)]);
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| fold(x) == fold(y))
}

/// `is_microsoft_internal_window_exe()` on origin/master.
fn model_internal(exe: Option<&[u8]>) -> bool {
    let Some(exe) = exe else { return false };
    INTERNAL_EXES_EXACT
        .iter()
        .any(|v| model_eqi_n(exe, v, usize::MAX))
        || INTERNAL_EXES_PARTIAL
            .iter()
            .any(|v| model_eqi_n(exe, v, v.len()))
}

/// `dstr_cmp(&exe, "explorer.exe") == 0 && dstr_is_empty(&title)`.
fn model_untitled_explorer(exe: Option<&[u8]>, title_empty: bool) -> bool {
    cstr(exe.unwrap_or(b"")) == EXPLORER_EXE && title_empty
}

/// `is_generic_class()`: `astrstri()` of each substring.
fn model_generic(class: Option<&[u8]>) -> bool {
    let Some(class) = class else { return false };
    let class = cstr(class);
    GENERIC_CLASS_SUBSTRINGS.iter().any(|find| {
        class
            .windows(find.len())
            .any(|w| model_eqi_n(w, find, find.len()))
    })
}

/// `is_uwp_class()`: two `strcmp()`s.
fn model_uwp(class: &[u8]) -> bool {
    UWP_CLASSES.iter().any(|&u| cstr(class) == u)
}

/// `window_rating()`'s score on origin/master, after its three compares.
/// Where the C is undefined (`abs(INT_MIN)`, and the penalty past
/// `INT_MAX`), this wraps; `astrcmpi()` never returns those.
fn model_rating(
    priority: i32,
    uwp: bool,
    generic: bool,
    class_matches: bool,
    exe_matches: bool,
    cmp: i32,
) -> i32 {
    let title_val = cmp.wrapping_abs();
    let priority = if generic && priority == PRIORITY_CLASS {
        PRIORITY_TITLE
    } else {
        priority
    };
    if uwp {
        if priority == PRIORITY_EXE && !exe_matches {
            NONE
        } else if title_val == 0 {
            0
        } else {
            NONE
        }
    } else if priority == PRIORITY_CLASS {
        let val = if class_matches { title_val } else { NONE };
        if val != NONE && !exe_matches {
            val.wrapping_add(PENALTY)
        } else {
            val
        }
    } else if priority == PRIORITY_TITLE {
        if title_val == 0 { 0 } else { NONE }
    } else if priority == PRIORITY_EXE {
        if exe_matches { title_val } else { NONE }
    } else {
        NONE
    }
}

/// Built names: each entry in each case pattern, with each one-byte
/// suffix and prefix from `extra`, and each one-byte cut.
fn variants(names: &[&[u8]], extra: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for name in names {
        for pattern in 0..4 {
            let n = recase(name, pattern);
            for &b in extra {
                out.push(cat(&n, &[b]));
                out.push(cat(&[b], &n));
            }
            for cut in 0..n.len() {
                out.push(n[..cut].to_vec());
                let mut gap = n.clone();
                gap.remove(cut);
                out.push(gap);
            }
            out.push(n);
        }
    }
    out
}

const EXTRA: &[u8] = b"\0 .xX_@[`{\x7f\x80\xc3\xff";

#[test]
fn edge_internal_against_model() {
    let mut names: Vec<&[u8]> = INTERNAL_EXES_EXACT.to_vec();
    names.extend(INTERNAL_EXES_PARTIAL);
    names.push(b"explorer.exe");
    for exe in variants(&names, EXTRA) {
        assert_eq!(
            is_internal_exe(Some(&exe)),
            model_internal(Some(&exe)),
            "{}",
            exe.escape_ascii()
        );
    }
    assert_eq!(is_internal_exe(None), model_internal(None));
}

#[test]
fn edge_untitled_explorer_against_model() {
    for exe in variants(&[b"explorer.exe"], EXTRA) {
        for title_empty in [false, true] {
            assert_eq!(
                is_untitled_explorer(Some(&exe), title_empty),
                model_untitled_explorer(Some(&exe), title_empty),
                "{}",
                exe.escape_ascii()
            );
        }
    }
    for title_empty in [false, true] {
        assert_eq!(
            is_untitled_explorer(None, title_empty),
            model_untitled_explorer(None, title_empty)
        );
    }
}

#[test]
fn edge_classes_against_model() {
    let names: [&[u8]; 4] = [b"Chrome", b"SDL_app", UWP_CLASSES[0], UWP_CLASSES[1]];
    for class in variants(&names, EXTRA) {
        for offset in 0..5 {
            let mut c = vec![b'q'; offset];
            c.extend(&class);
            assert_eq!(
                is_generic_class(Some(&c)),
                model_generic(Some(&c)),
                "{}",
                c.escape_ascii()
            );
            assert_eq!(is_uwp_class(&c), model_uwp(&c), "{}", c.escape_ascii());
        }
    }
    assert!(!is_generic_class(None));
}

/// Every priority (and some outside), the four flags and title results
/// over -300..=300 and at the ends of `int`. Past `i32::MAX - 0x1000` and
/// at `INT_MIN` the C is undefined; the core wraps, which the model pins.
#[test]
fn edge_rating_against_model() {
    let cmps = [
        i32::MAX - PENALTY,
        i32::MAX - PENALTY + 1,
        i32::MAX,
        i32::MIN + 1,
        i32::MIN,
    ];
    for p in [-2, -1, 0, 1, 2, 3, 4, i32::MIN, i32::MAX] {
        for flags in 0..16 {
            let (uwp, generic, class_matches, exe_matches) = (
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
                flags & 8 != 0,
            );
            for cmp in (-300..=300).chain(cmps) {
                assert_eq!(
                    rate(p, uwp, generic, class_matches, exe_matches, cmp),
                    model_rating(p, uwp, generic, class_matches, exe_matches, cmp),
                    "rating({p}, {uwp}, {generic}, {class_matches}, {exe_matches}, {cmp})"
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn edge_prop_internal(exe in proptest::collection::vec(any::<u8>(), 0..40)) {
        prop_assert_eq!(is_internal_exe(Some(&exe)), model_internal(Some(&exe)));
    }

    #[test]
    fn edge_prop_classes(class in proptest::collection::vec(any::<u8>(), 0..40)) {
        prop_assert_eq!(is_generic_class(Some(&class)), model_generic(Some(&class)));
        prop_assert_eq!(is_uwp_class(&class), model_uwp(&class));
    }

    #[test]
    fn edge_prop_rating(
        p in -4i32..8,
        uwp: bool,
        generic: bool,
        class_matches: bool,
        exe_matches: bool,
        cmp in -255i32..=255,
    ) {
        prop_assert_eq!(
            rate(p, uwp, generic, class_matches, exe_matches, cmp),
            model_rating(p, uwp, generic, class_matches, exe_matches, cmp)
        );
    }
}
