//! Tier 3 parity: the shim and the safe core against the original
//! `libobs/obs-output-caption-text.c.h` (oracle/output_caption_text.c).
//!
//! - The `CAPTION_*` bounds against the C's.
//! - The four helpers over every byte value (and every first two bytes,
//!   with the bytes after them drawn from the values the C tests against)
//!   against the C's static functions.
//! - `caption_text_row` against [`text_row`] on random text: the return
//!   value, every byte the C writes into its row buffer (both buffers start
//!   out the same, so a byte one side writes and the other does not shows)
//!   and the cursor.
//! - `obs_output_caption_text_page` against the shim, page after page until
//!   the text runs out, as `obs_output_output_caption_text2()` calls it:
//!   the return value, the whole page buffer and the cursor after each
//!   call, and the pages that loop queues. The shim runs with its own first
//!   window and with windows of 1 to 7 bytes, which make most of its calls
//!   retry with a larger one.
//! - The inputs: random bytes of every value, weighted toward whitespace,
//!   newlines, multibyte leads, continuation bytes and NUL, up to 20000
//!   bytes; runs of every lead byte; and the inputs of
//!   `caption_any_input_ends`.
//!
//! Every comparison also checks the bounds directly: a row of at most
//! `CAPTION_LINE_BYTES`, a page of at most `CAPTION_PAGE_BYTES`, and a
//! cursor that moves forward on a non-empty page and stays inside the text.
//!
//! Intentional differences: none. Nothing is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::c_char;

use obs_c_oracle::output_caption_text as oracle;
use obs_output::caption_text::{self, LINE_BYTES, LINE_CHARS, PAGE_BYTES, PAGE_ROWS, text_row};
use obs_output::ffi::output_caption_text::{
    FIRST_WINDOW, obs_output_caption_text_page, text_page_with_window,
};
use proptest::prelude::*;

use super::caption_text::weighted;

/// What both page buffers hold before a call.
const FILL: u8 = 0xA5;

/// The bytes the C tests the third and fourth byte of a sequence against.
const TAILS: [u8; 10] = [0x00, 0x41, 0x80, 0x8F, 0x90, 0x9F, 0xA0, 0xBF, 0xC0, 0xFF];

#[test]
fn parity_bounds() {
    let rust = [LINE_CHARS, PAGE_ROWS, LINE_BYTES, PAGE_BYTES];
    for (i, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter.
        assert_eq!(
            unsafe { oracle::oracle_output_caption_text_value(i) },
            value,
            "{name}"
        );
    }
}

#[test]
fn parity_helpers_every_byte() {
    for c in 0..=0xFFu8 {
        // SAFETY: plain functions of a byte or a NUL-terminated string.
        unsafe {
            assert_eq!(
                oracle::oracle_output_caption_text_utf8_continuation(c),
                caption_text::utf8_continuation(c)
            );
            let text = [c, 0];
            assert_eq!(
                oracle::oracle_output_caption_text_is_newline(text.as_ptr().cast()),
                caption_text::is_newline(&text)
            );
        }
    }
    for c0 in 0..=0xFFu8 {
        for c1 in 0..=0xFFu8 {
            for &c2 in &TAILS {
                for &c3 in &TAILS {
                    let text = [c0, c1, c2, c3, 0];
                    // The C reads only up to the NUL, so the Rust side gets
                    // the same string as a slice that ends there.
                    let end = text.iter().position(|&b| b == 0).unwrap_or(4);
                    let slice = &text[..end];
                    // SAFETY: a NUL-terminated string.
                    let c_len = unsafe {
                        oracle::oracle_output_caption_text_utf8_char_length(text.as_ptr().cast())
                    };
                    assert_eq!(c_len, caption_text::utf8_char_length(slice), "{text:02x?}");
                    if c2 != TAILS[2] || c3 != TAILS[2] {
                        continue;
                    }
                    for bytes in 0..=4 {
                        // Only lengths the string has, as the C is called.
                        if bytes > end {
                            break;
                        }
                        // SAFETY: as above; the C reads at most `bytes`.
                        let c_ws = unsafe {
                            oracle::oracle_output_caption_text_utf8_whitespace(
                                text.as_ptr().cast(),
                                bytes,
                            )
                        };
                        assert_eq!(
                            c_ws,
                            caption_text::utf8_whitespace(slice, bytes),
                            "{text:02x?} {bytes}"
                        );
                    }
                }
            }
        }
    }
}

/// `input` as the C sees it: NUL-terminated (it may hold NULs of its own,
/// which end the text for both sides).
fn c_string(input: &[u8]) -> Vec<u8> {
    let mut s = input.to_vec();
    s.push(0);
    s
}

/// The text the Rust core gets for the string at `at`: up to its NUL.
fn rest(s: &[u8], at: usize) -> &[u8] {
    let tail = &s[at..];
    &tail[..tail.iter().position(|&b| b == 0).unwrap_or(tail.len())]
}

/// `caption_text_row` against [`text_row`] from the start of each row,
/// past the 15 rows of a page, until the text runs out.
fn compare_rows(input: &[u8]) {
    let s = c_string(input);
    let mut at = 0usize;
    while s[at] != 0 {
        let mut c_row = [FILL; LINE_BYTES + 1];
        let mut cursor = s[at..].as_ptr().cast::<c_char>();
        // SAFETY: a NUL-terminated string and a CAPTION_LINE_BYTES + 1 row.
        let c_len = unsafe {
            oracle::oracle_output_caption_text_row(&mut cursor, c_row.as_mut_ptr().cast())
        };
        // SAFETY: the cursor moved within the same string.
        let c_moved = usize::try_from(unsafe { cursor.cast::<u8>().offset_from(s[at..].as_ptr()) })
            .expect("the C cursor moved forward");

        let row = text_row(rest(&s, at));
        let mut rust_row = [FILL; LINE_BYTES + 1];
        if !row.is_empty() {
            rust_row[..row.len()].copy_from_slice(row.as_bytes());
            rust_row[row.len()] = 0;
        }
        assert_eq!(c_len, row.len(), "row length at {at} of {input:02x?}");
        assert_eq!(c_row, rust_row, "row bytes at {at} of {input:02x?}");
        assert_eq!(
            c_moved,
            row.consumed(),
            "row cursor at {at} of {input:02x?}"
        );
        assert!(row.len() <= LINE_BYTES);
        let chars = row.as_bytes().iter().filter(|&&c| c & 0xC0 != 0x80).count();
        assert!(chars <= LINE_CHARS);

        // An empty row ends the text: only NULs follow its skipped bytes.
        if row.is_empty() {
            break;
        }
        at += row.consumed();
        if at >= s.len() {
            break;
        }
    }
}

/// How the shim is called.
#[derive(Clone, Copy, Debug)]
enum Shim {
    /// The exported function.
    Exported,
    /// The same with a first window of this many bytes.
    Window(usize),
}

/// One `obs_output_output_caption_text2()` loop over the oracle and over
/// the shim, compared after every call. Returns the queued pages.
fn compare_pages(input: &[u8], shim: Shim) -> Vec<Vec<u8>> {
    let s = c_string(input);
    let base = s.as_ptr();
    let mut c_cursor = base.cast::<c_char>();
    let mut r_cursor = base.cast::<c_char>();
    let mut pages = Vec::new();
    let mut queued = false;
    let mut calls = 0usize;

    loop {
        calls += 1;
        assert!(
            calls <= s.len(),
            "the cursor stopped moving in {input:02x?}"
        );
        let mut c_page = vec![FILL; PAGE_BYTES + 1];
        let mut r_page = vec![FILL; PAGE_BYTES + 1];
        // SAFETY: NUL-terminated strings and CAPTION_PAGE_BYTES + 1 pages.
        let (c_len, r_len) = unsafe {
            let c_len = oracle::oracle_obs_output_caption_text_page(
                &mut c_cursor,
                c_page.as_mut_ptr().cast(),
            );
            let r_len = match shim {
                Shim::Exported => {
                    obs_output_caption_text_page(&mut r_cursor, r_page.as_mut_ptr().cast())
                }
                Shim::Window(w) => {
                    text_page_with_window(&mut r_cursor, r_page.as_mut_ptr().cast(), w)
                }
            };
            (c_len, r_len)
        };
        // SAFETY: both cursors moved within `s`.
        let (c_at, r_at) = unsafe {
            (
                c_cursor.cast::<u8>().offset_from(base),
                r_cursor.cast::<u8>().offset_from(base),
            )
        };
        assert_eq!(
            c_len, r_len,
            "page length, call {calls}, {shim:?}, {input:02x?}"
        );
        assert_eq!(
            c_page, r_page,
            "page bytes, call {calls}, {shim:?}, {input:02x?}"
        );
        assert_eq!(c_at, r_at, "cursor, call {calls}, {shim:?}, {input:02x?}");

        let at = usize::try_from(r_at).expect("the cursor moved forward");
        assert!(at < s.len(), "the cursor left the string");
        assert!(r_len <= PAGE_BYTES);
        assert_eq!(0, r_page[r_len]);
        if r_len > 0 || !queued {
            pages.push(r_page[..r_len].to_vec());
            queued = true;
        }
        if s[at] == 0 {
            break;
        }
    }
    pages
}

/// Every comparison for one input.
fn compare_all(input: &[u8]) {
    let pages = compare_pages(input, Shim::Exported);
    for w in 1..=7 {
        assert_eq!(pages, compare_pages(input, Shim::Window(w)));
    }
    compare_rows(input);
}

#[test]
fn parity_characterization_inputs() {
    const POOL: [u8; 30] = [
        b' ', b'\t', b'\r', b'\n', 0x7F, 0x01, b'a', b'b', 0x80, 0xBF, 0xA0, 0xC2, 0xC3, 0xC0,
        0xC1, 0xE0, 0xE4, 0xED, 0xEF, 0xF0, 0xF4, 0xF5, 0xFF, 0x9F, 0x90, 0x8F, 0xB8, 0xAD, 0xA6,
        0x84,
    ];
    let fixed: [&[u8]; 12] = [
        b"",
        b"   \t\n  ",
        b"hello\tworld",
        b"aaaaaaaaaaaaaaaaaaaaaaaaa wrapped",
        b"one\rtwo\nthree\r\nfour",
        b"four\n\n\nfive\r\n\r\nsix",
        b"hello world \nnext ",
        b"a\x7Fb\x01c\x1Fd\x0Be\x0Cf",
        b"aaaa\xC2\xA0bbbb",
        b"one\xFFtwo",
        b"\xED\x9F\xBF|\xF4\x8F\xBF\xBF|\xE0\xA0\x80|\xF0\x90\x80\x80|\xC2\x80",
        b"ab\xE4\xB8",
    ];
    for input in fixed {
        compare_all(input);
    }
    for n in [127, 128, 129, 480, 481, 1000] {
        compare_all(&vec![b'a'; n]);
    }
    compare_all(&b"\xF0\x9F\xA6\x84".repeat(PAGE_ROWS * LINE_CHARS + 1));

    // every byte value 1-255, in order
    compare_all(&(1..=255u8).collect::<Vec<_>>());
    // a long run of each lead byte, with and without a continuation
    for lead in 0x80..=0xFFu8 {
        let input: Vec<u8> = (0..4096)
            .map(|i| if i & 1 == 1 { 0x80 } else { lead })
            .collect();
        compare_pages(&input, Shim::Exported);
        compare_pages(&input, Shim::Window(3));
        compare_rows(&input);
    }
    // the pseudo-random strings of caption_any_input_ends
    let mut seed: u32 = 0x2545_F491;
    for round in 0..64usize {
        let n = 1 + round * 61;
        let input: Vec<u8> = (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                POOL[(seed >> 16) as usize % POOL.len()]
            })
            .collect();
        compare_all(&input);
    }
}

#[test]
fn parity_runs_of_every_lead_byte() {
    // Each lead byte followed by runs of every continuation length, then
    // text, so every sequence length and truncation meets the wrap.
    for lead in 0..=0xFFu8 {
        for cont in [0x80u8, 0x8F, 0x90, 0x9F, 0xA0, 0xBF] {
            let mut input = Vec::new();
            for k in 0..=4 {
                for _ in 0..12 {
                    input.push(lead);
                    input.extend(std::iter::repeat_n(cont, k));
                    input.push(if k % 2 == 0 { b' ' } else { b'x' });
                }
            }
            compare_pages(&input, Shim::Exported);
            compare_pages(&input, Shim::Window(5));
            compare_rows(&input);
        }
    }
}

#[test]
fn parity_long_skips_cross_windows() {
    // Leading whitespace and invalid bytes far past the first window, and a
    // word after 32 characters of a row ending far past it.
    let mut input = vec![0xFFu8; FIRST_WINDOW * 3];
    input.extend(vec![b' '; FIRST_WINDOW * 2]);
    input.extend(b"x".repeat(31));
    input.extend(vec![0x80u8; FIRST_WINDOW * 5]);
    input.extend(b"y z");
    compare_all(&input);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1024)))]

    #[test]
    fn parity_weighted_text(input in weighted(800)) {
        compare_all(&input);
    }

    #[test]
    fn parity_raw_bytes(input in prop::collection::vec(any::<u8>(), 0..4000)) {
        compare_pages(&input, Shim::Exported);
        compare_pages(&input, Shim::Window(2));
        compare_rows(&input);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn parity_long_text(input in weighted(12000)) {
        compare_pages(&input, Shim::Exported);
        compare_pages(&input, Shim::Window(1));
    }
}
