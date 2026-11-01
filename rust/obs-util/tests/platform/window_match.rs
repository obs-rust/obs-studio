//! Tier 1: `obs_util::window_match`, the window helpers' pure decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_window_match_steps.c` 1:1:
//! its 20 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `ms_window_match_*` functions; here the
//! safe core is called directly.
//!
//! The `edge_*` cases are new: each decision against a model of the
//! origin/master C (its expressions inline, with ASCII folding written out,
//! and the codec as `dstr_replace()` and `strlist_split()` passes) over
//! built inputs, and proptests over arbitrary bytes and values.

use obs_util::window_match::{
    EXE_PENALTY, EXPLORER_EXE, GENERIC_CLASS_SUBSTRINGS, INTERNAL_EXES_EXACT,
    INTERNAL_EXES_PARTIAL, NO_MATCH, PRIORITY_CLASS, PRIORITY_EXE, PRIORITY_TITLE, UWP_CLASSES,
    decode, encode, encode_into, encoded_len, is_generic_class, is_internal_exe,
    is_untitled_explorer, is_uwp_class, rating, split,
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

/// The C test's `internal_names`, written out rather than taken from the
/// core's table.
const INTERNAL_NAMES: [&[u8]; 15] = [
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

#[test]
fn steps_internal_exact_names() {
    for name in INTERNAL_NAMES {
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

fn expect_encode(s: Option<&[u8]>, want: &[u8]) {
    let s = s.unwrap_or_default();
    let mut buf = [b'z'; 64];
    assert_eq!(encoded_len(s), want.len(), "encode({s:?})");
    assert_eq!(encode_into(s, &mut buf), want.len(), "encode({s:?})");
    assert_eq!(&buf[..want.len()], want, "encode({s:?})");
    assert_eq!(buf[want.len()], 0, "encode({s:?})");
    assert_eq!(encode(s), want, "encode({s:?})");
}

#[test]
fn steps_encode() {
    expect_encode(None, b"");
    expect_encode(Some(b""), b"");
    expect_encode(Some(b"abc"), b"abc");
    expect_encode(Some(b"#"), b"#22");
    expect_encode(Some(b":"), b"#3A");
    expect_encode(Some(b"::"), b"#3A#3A");
    expect_encode(Some(b"#:"), b"#22#3A");
    expect_encode(Some(b":#"), b"#3A#22");
    expect_encode(Some(b"#22"), b"#2222");
    expect_encode(Some(b"#3A"), b"#223A");
    expect_encode(Some(b"a:b:c"), b"a#3Ab#3Ac");
    expect_encode(Some(b"Title: x # y"), b"Title#3A x #22 y");
    expect_encode(Some(b"\xc3\xa9:\xff"), b"\xc3\xa9#3A\xff");
}

/// snprintf-style: the full length always, at most `out.len() - 1` bytes
/// and a NUL written, nothing at all into an empty `out`.
#[test]
fn steps_encode_truncates() {
    let mut buf = [b'z'; 8];

    assert_eq!(encode_into(b"a#b", &mut buf[..0]), 5);
    assert_eq!(buf[0], b'z');

    assert_eq!(encode_into(b"a#b", &mut buf[..1]), 5);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], b'z');

    buf = [b'z'; 8];
    assert_eq!(encode_into(b"a#b", &mut buf[..3]), 5);
    assert_eq!(&buf[..4], b"a#\0z");

    buf = [b'z'; 8];
    assert_eq!(encode_into(b"a#b", &mut buf[..5]), 5);
    assert_eq!(&buf[..6], b"a#22\0z");

    buf = [b'z'; 8];
    assert_eq!(encode_into(b"a#b", &mut buf[..6]), 5);
    assert_eq!(&buf[..7], b"a#22b\0z");

    buf = [b'z'; 8];
    assert_eq!(encode_into(b"", &mut buf), 0);
    assert_eq!(buf[0], 0);
    assert_eq!(buf[1], b'z');
}

fn expect_decode(s: &[u8], want: &[u8]) {
    let mut buf = s.to_vec();
    let n = decode(&mut buf);
    assert_eq!(n, want.len(), "decode({s:?})");
    assert_eq!(&buf[..n], want, "decode({s:?})");
}

/// "#3A" first, then "#22", each pass once, left to right.
#[test]
fn steps_decode() {
    // the C's NULL is 0; the core takes no Option
    assert_eq!(decode(&mut []), 0);
    expect_decode(b"", b"");
    expect_decode(b"abc", b"abc");
    expect_decode(b"#", b"#");
    expect_decode(b"#2", b"#2");
    expect_decode(b"#3", b"#3");
    expect_decode(b"#22", b"#");
    expect_decode(b"#3A", b":");
    expect_decode(b"#3A#3A", b"::");
    expect_decode(b"#3A3A", b":3A");
    expect_decode(b"#2222", b"#22");
    expect_decode(b"##22", b"##");
    expect_decode(b"##3A22", b"#:22");
    expect_decode(b"#3#3AA", b"#3:A");
    expect_decode(b"a#3Ab#22c", b"a:b#c");
    expect_decode(b"\xc3\xa9#3A\xff", b"\xc3\xa9:\xff");

    // the passes undo the encode's in reverse order: "#223A" is the
    // encoded "#3A", and a '#' the second pass makes is never part of a
    // ':'
    expect_decode(b"#223A", b"#3A");
    expect_decode(b"#22#3A", b"#:");

    // case-sensitive
    expect_decode(b"x#3a", b"x#3a");
    expect_decode(b"#3a", b"#3a");
}

fn expect_split(s: Option<&[u8]>, want: Option<[&[u8]; 3]>) {
    let got = s.and_then(split);
    match (got, want) {
        (None, None) => {}
        (Some(ranges), Some(want)) => {
            let s = s.unwrap_or_default();
            for (i, (range, want)) in ranges.into_iter().zip(want).enumerate() {
                assert_eq!(&s[range], want, "split({s:?}) field {i}");
            }
        }
        (got, want) => panic!("split({s:?}): {got:?}, expected {want:?}"),
    }
}

/// The first three fields at ':', empty ones kept, later ones ignored.
#[test]
fn steps_split() {
    expect_split(None, None);
    expect_split(Some(b""), None);
    expect_split(Some(b"a"), None);
    expect_split(Some(b":"), None);
    expect_split(Some(b"a:b"), None);
    expect_split(Some(b"::"), Some([b"", b"", b""]));
    expect_split(Some(b":::"), Some([b"", b"", b""]));
    expect_split(Some(b"a:b:c"), Some([b"a", b"b", b"c"]));
    expect_split(Some(b"a:b:c:d"), Some([b"a", b"b", b"c"]));
    expect_split(Some(b"a::c"), Some([b"a", b"", b"c"]));
    expect_split(Some(b"::c"), Some([b"", b"", b"c"]));
    expect_split(Some(b"a:b:"), Some([b"a", b"b", b""]));
    expect_split(
        Some(b"Title#3A x:Class:app.exe"),
        Some([b"Title#3A x", b"Class", b"app.exe"]),
    );
    expect_split(
        Some(b"\xc3\xa9:\xff:\x01"),
        Some([b"\xc3\xa9", b"\xff", b"\x01"]),
    );
}

const ROUND_TRIP_PARTS: [&[u8]; 13] = [
    b"",
    b"#",
    b":",
    b"#:",
    b":#",
    b"#22",
    b"#3A",
    b"#223A",
    b"##3A22",
    b"x#3a",
    b"a:b#c",
    b"Title: x # y",
    b"\xc3\xa9#:\xff",
];

/// The window list's `title:class:exe` of three parts.
fn join(parts: [&[u8]; 3]) -> Vec<u8> {
    parts.map(encode).join(&b':')
}

/// `add_window()`'s string, split and decoded as `ms_build_window_strings()`
/// does, gives each part back.
#[test]
fn steps_round_trip() {
    let n = ROUND_TRIP_PARTS.len();
    for t in 0..n {
        for c in 0..n {
            let parts = [
                ROUND_TRIP_PARTS[t],
                ROUND_TRIP_PARTS[c],
                ROUND_TRIP_PARTS[(t + c) % n],
            ];
            let joined = join(parts);
            let ranges = split(&joined).expect("three fields");
            for (range, want) in ranges.into_iter().zip(parts) {
                let mut field = joined[range].to_vec();
                assert!(!field.contains(&b':'), "{joined:?}");
                let len = decode(&mut field);
                assert_eq!(&field[..len], want, "round trip {joined:?}");
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

/// `dstr_replace()` on origin/master: every `find` from the left, the
/// scan going on after each `rep` it wrote.
fn model_replace(s: &[u8], find: &[u8], rep: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with(find) {
            out.extend_from_slice(rep);
            i += find.len();
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// `encode_dstr()` on origin/master.
fn model_encode(s: &[u8]) -> Vec<u8> {
    model_replace(&model_replace(cstr(s), b"#", b"#22"), b":", b"#3A")
}

/// `decode_str()` on origin/master.
fn model_decode(s: &[u8]) -> Vec<u8> {
    model_replace(&model_replace(cstr(s), b"#3A", b":"), b"#22", b"#")
}

/// `strlist_split(s, ':', true)` on origin/master, the first three fields
/// when there are three.
fn model_split(s: &[u8]) -> Option<[Vec<u8>; 3]> {
    let fields: Vec<&[u8]> = cstr(s).split(|&c| c == b':').collect();
    (fields.len() >= 3).then(|| [fields[0].to_vec(), fields[1].to_vec(), fields[2].to_vec()])
}

fn check_codec(s: &[u8]) {
    let want = model_encode(s);
    assert_eq!(encode(s), want, "encode({s:?})");
    assert_eq!(encoded_len(s), want.len(), "encode({s:?})");
    assert!(!want.contains(&b':'), "encode({s:?})");
    for size in [
        0,
        1,
        want.len() / 2,
        want.len(),
        want.len() + 1,
        want.len() + 2,
    ] {
        let mut out = vec![b'z'; size + 1];
        assert_eq!(
            encode_into(s, &mut out[..size]),
            want.len(),
            "encode({s:?}, {size})"
        );
        if let Some(last) = size.checked_sub(1) {
            let n = last.min(want.len());
            assert_eq!(&out[..n], &want[..n], "encode({s:?}, {size})");
            assert_eq!(out[n], 0, "encode({s:?}, {size})");
            assert!(
                out[n + 1..].iter().all(|&c| c == b'z'),
                "encode({s:?}, {size})"
            );
        } else {
            assert_eq!(out, [b'z'], "encode({s:?}, 0)");
        }
    }

    let mut buf = s.to_vec();
    let n = decode(&mut buf);
    let want = model_decode(s);
    assert_eq!(&buf[..n], want, "decode({s:?})");
    if n < buf.len() {
        assert_eq!(buf[n], 0, "decode({s:?})");
    }

    let mut field = encode(s);
    let n = decode(&mut field);
    assert_eq!(&field[..n], cstr(s), "round trip {s:?}");

    let got = split(s).map(|r| r.map(|r| s[r].to_vec()));
    assert_eq!(got, model_split(s), "split({s:?})");
}

/// Every string up to 5 bytes over the bytes the codec looks at.
#[test]
fn edge_codec_against_model() {
    const ALPHA: &[u8] = b"#:23Aa\x00\xc3";
    let mut s = Vec::new();
    for n in 0..=5u32 {
        for mut k in 0..ALPHA.len().pow(n) {
            s.clear();
            for _ in 0..n {
                s.push(ALPHA[k % ALPHA.len()]);
                k /= ALPHA.len();
            }
            check_codec(&s);
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

    #[test]
    fn edge_prop_codec(
        s in proptest::collection::vec(
            prop_oneof![
                4 => proptest::sample::select(b"#:23A".to_vec()),
                1 => any::<u8>(),
            ],
            0..48,
        ),
    ) {
        check_codec(&s);
        let parts = [s.as_slice(), b"cls:#".as_slice(), s.as_slice()];
        let joined = join(parts);
        let ranges = split(&joined).expect("three fields");
        for (range, want) in ranges.into_iter().zip(parts) {
            let mut field = joined[range].to_vec();
            let n = decode(&mut field);
            prop_assert_eq!(&field[..n], cstr(want));
        }
    }
}
