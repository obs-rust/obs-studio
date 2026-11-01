//! obs-util: the window helpers' pure decisions
//! (`libobs/util/windows/window-match.c.h`). Filling the window list asks
//! the internal-exe test of every window and the untitled-Explorer test of
//! the rest; a search asks the two class tests once and rates every window.
//! Each side runs 32 windows (internal, Explorer, games, browsers and UWP
//! apps, in mixed case) through the list's tests, and rates them under the
//! three priorities against a saved generic, UWP and plain class, folded
//! into a checksum both sides must match. The codec pair builds each
//! window's saved `title:class:exe` string, as the list does, and splits
//! and decodes it again, as a capture source does when it loads, in stack
//! buffers. Nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::window_match::{
    oracle_ms_window_match_decode, oracle_ms_window_match_encode, oracle_ms_window_match_is_generic_class,
    oracle_ms_window_match_is_internal_exe, oracle_ms_window_match_is_untitled_explorer,
    oracle_ms_window_match_is_uwp_class, oracle_ms_window_match_rating, oracle_ms_window_match_split,
};
use obs_util::ffi::window_match::{
    ms_window_match_decode, ms_window_match_encode, ms_window_match_is_generic_class,
    ms_window_match_is_internal_exe, ms_window_match_is_untitled_explorer, ms_window_match_is_uwp_class,
    ms_window_match_rating, ms_window_match_split,
};
use std::ffi::{CStr, c_char};
use std::hint::black_box;

const EXES: [&CStr; 8] = [
    c"ShellExperienceHost.exe",
    c"explorer.exe",
    c"Explorer.EXE",
    c"game.exe",
    c"chrome.exe",
    c"WindowsInternal.ComposableShell.Experiences.TextInput.InputApp.exe",
    c"obs64.exe",
    c"time.exe",
];
const CLASSES: [&CStr; 3] = [c"Chrome_WidgetWin_1", c"Windows.UI.Core.CoreWindow", c"UnrealWindow"];

fn exe(i: usize) -> *const c_char {
    EXES[i % EXES.len()].as_ptr()
}

fn mix(acc: u32, value: u32) -> u32 {
    acc.wrapping_mul(31).wrapping_add(value)
}

/// One pass over the 32 windows with each side's five functions.
macro_rules! windows {
    ($internal:ident, $explorer:ident, $generic:ident, $uwp:ident, $rating:ident) => {{
        let mut acc = 0_u32;
        for i in 0..32_usize {
            let e = black_box(exe(i));
            // SAFETY: NUL-terminated literals.
            let listed = unsafe { !$internal(e) && !$explorer(e, i % 3 == 0) };
            acc = mix(acc, u32::from(listed));
        }
        for class in CLASSES {
            let class = black_box(class.as_ptr());
            // SAFETY: NUL-terminated literals.
            let (uwp, generic) = unsafe { ($uwp(class), $generic(class)) };
            for priority in 0..3 {
                for i in 0..32_i32 {
                    // SAFETY: plain values. The Rust shim is safe to call;
                    // the oracle's binding is not.
                    #[allow(unused_unsafe)]
                    let r = unsafe {
                        $rating(priority, uwp, generic, i % 4 == 0, i % 3 != 0, black_box((i % 9) - 4))
                    };
                    acc = mix(acc, r.cast_unsigned());
                }
            }
        }
        acc
    }};
}

fn rust_windows() -> u32 {
    windows!(
        ms_window_match_is_internal_exe,
        ms_window_match_is_untitled_explorer,
        ms_window_match_is_generic_class,
        ms_window_match_is_uwp_class,
        ms_window_match_rating
    )
}

fn c_windows() -> u32 {
    windows!(
        oracle_ms_window_match_is_internal_exe,
        oracle_ms_window_match_is_untitled_explorer,
        oracle_ms_window_match_is_generic_class,
        oracle_ms_window_match_is_uwp_class,
        oracle_ms_window_match_rating
    )
}

const TITLES: [&CStr; 4] = [c"OBS 31.0: Scenes # 2", c"Untitled - Notepad", c"#3A#22: x", c"game"];

/// One pass over the 32 windows' saved strings with each side's codec:
/// encode the title, class and exe into one string, split it, and decode
/// each field in place.
macro_rules! codec {
    ($encode:ident, $split:ident, $decode:ident) => {{
        let mut acc = 0_u32;
        for i in 0..32_usize {
            let parts = [
                TITLES[i % TITLES.len()].as_ptr(),
                CLASSES[i % CLASSES.len()].as_ptr(),
                exe(i),
            ];
            let mut joined = [0 as c_char; 256];
            let mut pos = 0_usize;
            for (n, part) in parts.into_iter().enumerate() {
                if n > 0 {
                    joined[pos] = b':' as c_char;
                    pos += 1;
                }
                // SAFETY: a NUL-terminated literal into the rest of the
                // stack buffer.
                #[allow(unused_unsafe)]
                let len = unsafe {
                    $encode(black_box(part), joined.as_mut_ptr().add(pos), joined.len() - pos)
                };
                pos += len;
            }
            let (mut start, mut len) = ([0_usize; 3], [0_usize; 3]);
            // SAFETY: the NUL-terminated string just built, three outputs each.
            #[allow(unused_unsafe)]
            let ok = unsafe { $split(joined.as_ptr(), start.as_mut_ptr(), len.as_mut_ptr()) };
            acc = mix(acc, u32::from(ok));
            for f in 0..3 {
                let mut field = [0 as c_char; 256];
                field[..len[f]].copy_from_slice(&joined[start[f]..start[f] + len[f]]);
                // SAFETY: a NUL-terminated copy of the field.
                #[allow(unused_unsafe)]
                let n = unsafe { $decode(field.as_mut_ptr()) };
                acc = mix(acc, n as u32);
            }
        }
        acc
    }};
}

fn rust_codec() -> u32 {
    codec!(ms_window_match_encode, ms_window_match_split, ms_window_match_decode)
}

fn c_codec() -> u32 {
    codec!(oracle_ms_window_match_encode, oracle_ms_window_match_split, oracle_ms_window_match_decode)
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "window_match/windows/32", rust_windows, c_windows);
    pair(criterion, "window_match/codec/32", rust_codec, c_codec);
}
