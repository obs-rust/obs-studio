//! Tier 1: a 1:1 port of `test/cmocka/test_output_caption_text.c` against
//! the safe core [`obs_output::caption_text`], plus edge cases.
//!
//! The C test drives `obs_output_output_caption_text2()` and walks the
//! queued pages. That function stays C; [`queue`] repeats its loop over
//! [`text_page`] (one page per call until the text runs out, and only a
//! sole page may be empty), so each case below checks what the C test
//! checks, in the same order, under the same name.

use obs_output::caption_text::{
    self, LINE_BYTES, LINE_CHARS, PAGE_BYTES, PAGE_ROWS, text_page, text_row,
};
use proptest::prelude::*;

/// The byte at `i`, or the NUL that ends the text.
fn byte(text: &[u8], i: usize) -> u8 {
    text.get(i).copied().unwrap_or(0)
}

/// The pages `obs_output_output_caption_text2()` queues for `text`.
fn queue(text: &[u8]) -> Vec<Vec<u8>> {
    let mut pages = Vec::new();
    let mut cursor = 0usize;
    let mut queued = false;
    let mut calls = 0usize;

    loop {
        calls += 1;
        assert!(
            calls <= text.len() + 1,
            "the cursor stopped moving at {cursor} of {}",
            text.len()
        );
        let page = text_page(text.get(cursor..).unwrap_or_default());
        cursor += page.consumed();
        assert!(cursor <= text.len(), "the cursor ran past the text");
        if !page.is_empty() || !queued {
            pages.push(page.as_bytes().to_vec());
            queued = true;
        }
        if byte(text, cursor) == 0 {
            break;
        }
    }
    pages
}

/// `assert_valid_utf8`: continuation bytes only after a valid lead byte,
/// and no truncated sequence at the end.
fn assert_valid_utf8(text: &[u8]) {
    let mut i = 0usize;
    while let Some(&c) = text.get(i) {
        let len = if c < 0x80 {
            1
        } else if c & 0xE0 == 0xC0 {
            2
        } else if c & 0xF0 == 0xE0 {
            3
        } else if c & 0xF8 == 0xF0 {
            4
        } else {
            0
        };
        assert!(len > 0, "invalid lead byte {c:#04x} in {text:02x?}");
        for k in 1..len {
            assert_eq!(
                byte(text, i + k) & 0xC0,
                0x80,
                "missing continuation in {text:02x?}"
            );
        }
        i += len;
    }
}

fn is_c_space(c: u8) -> bool {
    matches!(c, b'\n' | b' ' | b'\r' | b'\t')
}

/// `assert_pages_preserve`: every byte of the input but `\n`, space, `\r`
/// and `\t` survives, in order.
fn assert_pages_preserve(pages: &[Vec<u8>], input: &[u8]) {
    let mut joined = Vec::new();
    for page in pages.iter().take(8) {
        assert_valid_utf8(page);
        joined.extend(page.iter().copied().filter(|&c| !is_c_space(c)));
    }
    let expected: Vec<u8> = input.iter().copied().filter(|&c| !is_c_space(c)).collect();
    assert_eq!(expected, joined);
}

/// `assert_single_page`: exactly one page with this text.
fn assert_single_page(input: &[u8], expected: &[u8]) {
    let pages = queue(input);
    assert_eq!(1, pages.len(), "pages of {input:02x?}: {pages:02x?}");
    assert_eq!(expected, pages[0].as_slice(), "input {input:02x?}");
}

/// `assert_page_invariants`: at most `CAPTION_PAGE_BYTES`, 1-15 rows joined
/// by `\n`, each row 1 to `CAPTION_LINE_BYTES` bytes and at most
/// `CAPTION_LINE_CHARS` characters, starting with a non-whitespace
/// character, with no CR or LF inside, and well-formed UTF-8. Only a sole
/// page may be empty. Returns the page count.
fn assert_page_invariants(pages: &[Vec<u8>]) -> usize {
    for page in pages {
        assert!(page.len() <= PAGE_BYTES);
        assert_valid_utf8(page);
        if page.is_empty() {
            assert_eq!(1, pages.len());
            continue;
        }
        let mut rows = 0usize;
        for row in page.split(|&c| c == b'\n') {
            rows += 1;
            assert!(!row.is_empty());
            assert!(row.len() <= LINE_BYTES);
            assert!(
                row[0] > b' ' && row[0] != 0x7F,
                "row starts with {row:02x?}"
            );
            assert!(!(row.len() >= 2 && row[0] == 0xC2 && row[1] == 0xA0));
            assert!(!row.contains(&b'\r'));
            let chars = row.iter().filter(|&&c| c & 0xC0 != 0x80).count();
            assert!(chars <= LINE_CHARS);
        }
        assert!(rows <= PAGE_ROWS);
    }
    pages.len()
}

fn repeat(n: usize, c: u8) -> Vec<u8> {
    vec![c; n]
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// `strstr`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

// --- The 32 cases of test_output_caption_text, in order. ------------------

#[test]
fn caption_boundary_127() {
    let input = repeat(127, b'a');
    assert_pages_preserve(&queue(&input), &input);
}

#[test]
fn caption_boundary_128() {
    let input = repeat(128, b'a');
    assert_pages_preserve(&queue(&input), &input);
}

#[test]
fn caption_boundary_129() {
    let input = repeat(129, b'a');
    assert_pages_preserve(&queue(&input), &input);
}

#[test]
fn caption_multibyte_at_128() {
    let input = cat(&[&repeat(127, b'a'), b"\xC3\xA9", b"tail"]);
    let pages = queue(&input);
    assert_pages_preserve(&pages, &input);
    assert!(!pages.is_empty());
    assert!(contains(pages.last().unwrap(), b"tail"));
}

#[test]
fn caption_multi_page() {
    let mut input = Vec::new();
    for i in 0..40 {
        if i > 0 {
            input.push(b' ');
        }
        input.extend(format!("row{i:02}-abcdefghijklmnopqrstuvwx").bytes());
    }
    let pages = queue(&input);
    assert_eq!(3, pages.len());
    for i in 0..40 {
        let marker = format!("row{i:02}");
        let expected_page = i / 15;
        for (p, page) in pages.iter().enumerate() {
            assert_eq!(p == expected_page, contains(page, marker.as_bytes()));
        }
    }
}

#[test]
fn caption_word_wrap() {
    let pages = queue(b"aaaaaaaaaaaaaaaaaaaaaaaaa wrapped");
    assert_eq!(1, pages.len());
    assert_eq!(b"aaaaaaaaaaaaaaaaaaaaaaaaa\nwrapped".as_slice(), pages[0]);
}

#[test]
fn caption_page_edge_480() {
    let input = repeat(480, b'a');
    let pages = queue(&input);
    assert_pages_preserve(&pages, &input);
    assert_eq!(1, pages.len());
}

#[test]
fn caption_page_edge_481() {
    let input = repeat(481, b'a');
    let pages = queue(&input);
    assert_pages_preserve(&pages, &input);
    assert_eq!(2, pages.len());
    assert_eq!(b"a".as_slice(), pages[1]);
}

#[test]
fn caption_utf8_3byte_at_edge() {
    let input = cat(&[&repeat(31, b'a'), b"\xE4\xB8\xAD", b"b"]);
    let pages = queue(&input);
    assert_pages_preserve(&pages, &input);
    assert_eq!(1, pages.len());
    assert!(contains(&pages[0], b"\xE4\xB8\xAD\nb"));
}

#[test]
fn caption_utf8_4byte_at_edge() {
    let input = cat(&[&repeat(31, b'a'), b"\xF0\x9F\xA6\x84", b"b"]);
    let pages = queue(&input);
    assert_pages_preserve(&pages, &input);
    assert_eq!(1, pages.len());
    assert!(contains(&pages[0], b"\xF0\x9F\xA6\x84\nb"));
}

#[test]
fn caption_whitespace_only() {
    let pages = queue(b"   \t\n  ");
    assert_eq!(1, pages.len());
    assert_eq!(b"".as_slice(), pages[0]);
}

#[test]
fn caption_tab_becomes_space() {
    let pages = queue(b"hello\tworld");
    assert_eq!(1, pages.len());
    assert_eq!(b"hello world".as_slice(), pages[0]);
}

#[test]
fn caption_invalid_utf8_dropped() {
    // a lone continuation byte
    assert_single_page(b"a\x80b", b"ab");
    // C0 and C1 overlong leads
    assert_single_page(b"a\xC0\xAFb\xC1\xBFc", b"abc");
    // E0 80-9F: overlong 3-byte forms
    assert_single_page(b"a\xE0\x80\xAFb\xE0\x9F\xBFc", b"abc");
    // ED A0-BF: UTF-16 surrogates
    assert_single_page(b"a\xED\xA0\x80b\xED\xBF\xBFc", b"abc");
    // F0 80-8F: overlong 4-byte forms
    assert_single_page(b"a\xF0\x8F\xBF\xBFb", b"ab");
    // F4 90 and above: past U+10FFFF
    assert_single_page(b"a\xF4\x90\x80\x80b", b"ab");
    // F5-FF never start a character
    assert_single_page(b"a\xF5\x80\x80\x80b\xF8\x80c\xFE\xFFd", b"abcd");
    // sequences truncated by the end of the string
    assert_single_page(b"ab\xE4\xB8", b"ab");
    assert_single_page(b"ab\xF0\x9F\xA6", b"ab");
    assert_single_page(b"ab\xC3", b"ab");
    // a sequence truncated by an ASCII byte
    assert_single_page(b"a\xE4\xB8b", b"ab");
    // the valid edges of each range are kept: U+D7FF, U+10FFFF, U+0800,
    // U+10000, U+0080
    let edges = b"\xED\x9F\xBF|\xF4\x8F\xBF\xBF|\xE0\xA0\x80|\xF0\x90\x80\x80|\xC2\x80";
    assert_single_page(edges, edges);
}

/// Characterized, not endorsed: an invalid byte is dropped, not replaced,
/// so the words on either side of it join.
#[test]
fn caption_invalid_byte_joins_words() {
    assert_single_page(b"one\xFFtwo", b"onetwo");
}

#[test]
fn caption_invalid_bytes_take_no_column() {
    let mut input = Vec::new();
    let mut expected = Vec::new();
    for _ in 0..32 {
        input.extend_from_slice(b"\xF0\x9F\xA6\x84\x80");
        expected.extend_from_slice(b"\xF0\x9F\xA6\x84");
    }
    input.push(b'z');
    expected.extend_from_slice(b"\nz");
    assert_single_page(&input, &expected);
}

#[test]
fn caption_only_invalid_bytes() {
    assert_single_page(b"\xFF\xFE\x80\xC0", b"");
    assert_single_page(b"\xE4\xB8", b"");
}

#[test]
fn caption_empty_string() {
    assert_single_page(b"", b"");
}

#[test]
fn caption_all_spaces() {
    assert_single_page(b" ", b"");
    assert_single_page(b"        ", b"");
    assert_single_page(&repeat(100, b' '), b"");
}

#[test]
fn caption_newlines_end_rows() {
    assert_single_page(b"one\rtwo\nthree\r\nfour", b"one\ntwo\nthree\nfour");
}

/// Characterized, not endorsed: a newline is skipped as leading
/// whitespace, so blank lines (and leading and trailing newlines) give no
/// empty rows.
#[test]
fn caption_blank_lines_collapse() {
    assert_single_page(b"four\n\n\nfive\r\n\r\nsix", b"four\nfive\nsix");
    assert_single_page(b"\r\n\nhello\n\r", b"hello");
}

/// Characterized, not endorsed: whitespace before a newline or the end of
/// the text stays in the row; only a wrap drops the whitespace it splits
/// at.
#[test]
fn caption_trailing_whitespace_kept() {
    assert_single_page(b"hello world \nnext ", b"hello world \nnext ");
    assert_single_page(b"end\t", b"end ");
}

#[test]
fn caption_nbsp() {
    assert_single_page(b"aaaa\xC2\xA0bbbb", b"aaaa\xC2\xA0bbbb");
    assert_single_page(b"\xC2\xA0\xC2\xA0hi", b"hi");

    // the wrap splits at the NBSP and drops it
    let input = cat(&[&repeat(30, b'a'), b"\xC2\xA0bbbbb"]);
    let expected = cat(&[&repeat(30, b'a'), b"\nbbbbb"]);
    assert_single_page(&input, &expected);

    // an NBSP as character 33 ends a full row
    let input = cat(&[&repeat(32, b'a'), b"\xC2\xA0b"]);
    let expected = cat(&[&repeat(32, b'a'), b"\nb"]);
    assert_single_page(&input, &expected);
}

/// Characterized, not endorsed: every C0 control byte and DEL counts as
/// whitespace, so inside a row each one becomes a space.
#[test]
fn caption_controls_become_spaces() {
    assert_single_page(b"a\x7Fb\x01c\x1Fd\x0Be\x0Cf", b"a b c d e f");
    assert_single_page(b"\x7F\x01lead", b"lead");
}

#[test]
fn caption_word_of_32() {
    let word = repeat(32, b'x');
    assert_single_page(&word, &word);
    assert_single_page(&cat(&[&word, b" next"]), &cat(&[&word, b"\nnext"]));
}

#[test]
fn caption_word_of_33() {
    let word32 = repeat(32, b'x');
    let word33 = repeat(33, b'x');
    assert_single_page(&word33, &cat(&[&word32, b"\nx"]));
    assert_single_page(&cat(&[b"ab ", &word33]), &cat(&[b"ab\n", &word32, b"\nx"]));
}

#[test]
fn caption_space_at_column_32() {
    let word31 = repeat(31, b'a');
    assert_single_page(&cat(&[&word31, b" b"]), &cat(&[&word31, b"\nb"]));
}

/// Characterized, not endorsed: with whitespace as characters 32 and 33,
/// the wrap splits at the later one, so the row ends with a space.
#[test]
fn caption_two_spaces_at_column_32() {
    let word31 = repeat(31, b'a');
    assert_single_page(&cat(&[&word31, b"  b"]), &cat(&[&word31, b" \nb"]));
}

#[test]
fn caption_utf8_2byte_at_wrap() {
    let input = cat(&[&repeat(31, b'a'), b"\xC3\xA9\xC3\xA9"]);
    let expected = cat(&[&repeat(31, b'a'), b"\xC3\xA9\n\xC3\xA9"]);
    assert_single_page(&input, &expected);
}

#[test]
fn caption_row_and_page_byte_bound() {
    let chars = PAGE_ROWS * LINE_CHARS;
    let mut input = b"\xF0\x9F\xA6\x84".repeat(chars);

    let pages = queue(&input);
    assert_eq!(1, pages.len());
    assert_eq!(PAGE_BYTES, pages[0].len());
    assert_eq!(1, assert_page_invariants(&pages));

    input.push(b'z');
    let pages = queue(&input);
    assert_eq!(2, pages.len());
    assert_eq!(PAGE_BYTES, pages[0].len());
    assert_eq!(b"z".as_slice(), pages[1]);
    assert_eq!(2, assert_page_invariants(&pages));
}

#[test]
fn caption_more_than_15_rows() {
    let mut input = Vec::new();
    let mut expected = Vec::new();
    for i in 0..16 {
        let line = format!("{}l{i:02}", if i > 0 { "\n" } else { "" });
        input.extend(line.bytes());
        if i < 15 {
            expected.extend(line.bytes());
        }
    }
    let pages = queue(&input);
    assert_eq!(2, pages.len());
    assert_eq!(expected, pages[0]);
    assert_eq!(b"l15".as_slice(), pages[1]);
}

#[test]
fn caption_full_page_then_whitespace() {
    let input = cat(&[
        &repeat(PAGE_ROWS * LINE_CHARS, b'a'),
        b"  \r\n\t\xC2\xA0\xFF\x80 ",
    ]);
    let pages = queue(&input);
    assert_eq!(1, pages.len());
    assert_eq!(PAGE_ROWS * LINE_CHARS + PAGE_ROWS - 1, pages[0].len());
}

#[test]
fn caption_any_input_ends() {
    const POOL: [u8; 30] = [
        b' ', b'\t', b'\r', b'\n', 0x7F, 0x01, b'a', b'b', 0x80, 0xBF, 0xA0, 0xC2, 0xC3, 0xC0,
        0xC1, 0xE0, 0xE4, 0xED, 0xEF, 0xF0, 0xF4, 0xF5, 0xFF, 0x9F, 0x90, 0x8F, 0xB8, 0xAD, 0xA6,
        0x84,
    ];
    let len = 4096usize;
    let mut seed: u32 = 0x2545_F491;

    // every byte value 1-255, in order
    let input: Vec<u8> = (1..=255u8).collect();
    assert!(assert_page_invariants(&queue(&input)) >= 1);

    // a long run of each lead byte, with and without a continuation
    for lead in 0x80..=0xFFu8 {
        let input: Vec<u8> = (0..len)
            .map(|i| if i & 1 == 1 { 0x80 } else { lead })
            .collect();
        assert!(assert_page_invariants(&queue(&input)) >= 1);
    }

    // pseudo-random strings of several lengths
    for round in 0..64usize {
        let n = 1 + round * 61;
        let input: Vec<u8> = (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                POOL[(seed >> 16) as usize % POOL.len()]
            })
            .collect();
        assert!(assert_page_invariants(&queue(&input)) >= 1);
    }
}

// --- Edge cases. -----------------------------------------------------------

#[test]
fn constants_match_obs_internal_h() {
    assert_eq!(32, LINE_CHARS);
    assert_eq!(15, PAGE_ROWS);
    assert_eq!(128, LINE_BYTES);
    assert_eq!(15 * 128 + 14, PAGE_BYTES);
}

#[test]
fn char_length_of_every_lead_byte() {
    for lead in 0..=0xFFu8 {
        // with three continuation bytes that suit every lead
        let text = [lead, 0x90, 0x80, 0x80];
        let expected = match lead {
            0 => 0,
            0x01..=0x7F => 1,
            0xC2..=0xDF => 2,
            // E0 needs A0-BF, ED needs 80-9F: 0x90 suits ED but not E0
            0xE0 => 0,
            0xE1..=0xEF => 3,
            // F0 needs 90-BF, F4 needs 80-8F: 0x90 suits F0 but not F4
            0xF0..=0xF3 => 4,
            _ => 0,
        };
        assert_eq!(
            expected,
            caption_text::utf8_char_length(&text),
            "{lead:#04x}"
        );
    }
}

#[test]
fn char_length_range_edges() {
    use caption_text::utf8_char_length as len;
    assert_eq!(0, len(b""));
    assert_eq!(0, len(b"\0a"));
    assert_eq!(1, len(b"\x7F"));
    assert_eq!(0, len(b"\x80"));
    assert_eq!(0, len(b"\xC1\xBF"));
    assert_eq!(2, len(b"\xC2\x80"));
    assert_eq!(2, len(b"\xDF\xBF"));
    assert_eq!(0, len(b"\xC2\xC0"));
    assert_eq!(0, len(b"\xE0\x9F\xBF"));
    assert_eq!(3, len(b"\xE0\xA0\x80"));
    assert_eq!(3, len(b"\xED\x9F\xBF"));
    assert_eq!(0, len(b"\xED\xA0\x80"));
    assert_eq!(3, len(b"\xEF\xBF\xBF"));
    assert_eq!(0, len(b"\xF0\x8F\xBF\xBF"));
    assert_eq!(4, len(b"\xF0\x90\x80\x80"));
    assert_eq!(4, len(b"\xF4\x8F\xBF\xBF"));
    assert_eq!(0, len(b"\xF4\x90\x80\x80"));
    assert_eq!(0, len(b"\xF5\x80\x80\x80"));
    // truncated by the end of the slice, by a NUL and by an ASCII byte
    assert_eq!(0, len(b"\xE4\xB8"));
    assert_eq!(0, len(b"\xE4\xB8\0"));
    assert_eq!(0, len(b"\xF0\x9F\xA6a"));
    // a sequence followed by more text
    assert_eq!(2, len(b"\xC3\xA9abc"));
}

#[test]
fn continuation_bytes() {
    for c in 0..=0xFFu8 {
        assert_eq!(
            (0x80..=0xBF).contains(&c),
            caption_text::utf8_continuation(c)
        );
    }
}

#[test]
fn whitespace_and_newline() {
    use caption_text::{is_newline, utf8_whitespace};
    for c in 0..=0xFFu8 {
        assert_eq!(c <= b' ' || c == 0x7F, utf8_whitespace(&[c], 1), "{c:#04x}");
        assert_eq!(c == b'\r' || c == b'\n', is_newline(&[c]), "{c:#04x}");
    }
    assert!(utf8_whitespace(b"\xC2\xA0", 2));
    assert!(!utf8_whitespace(b"\xC2\xA1", 2));
    assert!(!utf8_whitespace(b"\xC3\xA0", 2));
    // the length decides: a space read as 2 bytes is not whitespace
    assert!(!utf8_whitespace(b" a", 2));
    assert!(!utf8_whitespace(b"\xC2\xA0", 1));
    assert!(!utf8_whitespace(b"\xE3\x80\x80", 3));
    // the end of the text reads as NUL, which is a 1-byte "whitespace"
    assert!(utf8_whitespace(b"", 1));
    assert!(!is_newline(b""));
}

#[test]
fn row_cursor() {
    // leading whitespace and invalid bytes are consumed even when no row
    // is left
    let row = text_row(b" \t\xFF\r\n");
    assert!(row.is_empty());
    assert_eq!(5, row.consumed());
    assert!(row.reached_end());

    // the cursor stops at the whitespace a wrap splits at
    let row = text_row(b"  aaaaaaaaaaaaaaaaaaaaaaaaa wrapped");
    assert_eq!(b"aaaaaaaaaaaaaaaaaaaaaaaaa".as_slice(), row.as_bytes());
    assert_eq!(27, row.consumed());
    assert!(!row.reached_end());

    // and at the newline that ends a row
    let row = text_row(b"one\r\ntwo");
    assert_eq!(b"one".as_slice(), row.as_bytes());
    assert_eq!(3, row.consumed());

    // and after character 32 of a hard wrap
    let row = text_row(&repeat(40, b'x'));
    assert_eq!(repeat(32, b'x'), row.as_bytes());
    assert_eq!(32, row.consumed());
    assert!(!row.reached_end());

    // a text that ends inside the row
    let row = text_row(b"end ");
    assert_eq!(b"end ".as_slice(), row.as_bytes());
    assert_eq!(4, row.consumed());
    assert!(row.reached_end());
}

#[test]
fn row_ends_at_nul() {
    let row = text_row(b"ab\0cd");
    assert_eq!(b"ab".as_slice(), row.as_bytes());
    assert_eq!(2, row.consumed());
    // a NUL in the slice is the end of the text, not the end of the slice
    assert!(!row.reached_end());

    let page = text_page(b"ab\ncd\0ef");
    assert_eq!(b"ab\ncd".as_slice(), page.as_bytes());
    assert_eq!(5, page.consumed());
    assert!(!page.reached_end());
}

#[test]
fn row_skips_invalid_bytes_after_column_32() {
    // the wrap still splits at the space after the dropped bytes
    let input = cat(&[&repeat(32, b'a'), b"\xFF\x80 b"]);
    let row = text_row(&input);
    assert_eq!(repeat(32, b'a'), row.as_bytes());
    assert_eq!(34, row.consumed());
    assert_single_page(&input, &cat(&[&repeat(32, b'a'), b"\nb"]));

    // invalid bytes before a newline are dropped and the row ends there
    assert_single_page(b"ab\xFF\x80\ncd", b"ab\ncd");
}

#[test]
fn row_wraps_at_the_last_whitespace() {
    let input = b"aaa bbb ccc ddd eee fff ggg hhh iii";
    let row = text_row(input);
    assert_eq!(
        b"aaa bbb ccc ddd eee fff ggg hhh".as_slice(),
        row.as_bytes()
    );
    assert_eq!(31, row.consumed());
    // a tab at the split is dropped too
    assert_single_page(
        &cat(&[&repeat(20, b'a'), b"\t", &repeat(20, b'b')]),
        &cat(&[&repeat(20, b'a'), b"\n", &repeat(20, b'b')]),
    );
}

#[test]
fn row_of_32_four_byte_characters() {
    let input = b"\xF0\x9F\xA6\x84".repeat(33);
    let row = text_row(&input);
    assert_eq!(LINE_BYTES, row.len());
    assert_eq!(LINE_BYTES, row.consumed());
}

#[test]
fn page_cursor() {
    // fifteen rows, then the cursor stays at the newline before row 16
    let mut input = Vec::new();
    for i in 0..16 {
        if i > 0 {
            input.push(b'\n');
        }
        input.extend(format!("l{i:02}").bytes());
    }
    let page = text_page(&input);
    assert_eq!(15 * 3 + 14, page.len());
    assert_eq!(15 * 4 - 1, page.consumed());
    assert!(!page.reached_end());
    let next = text_page(input.get(page.consumed()..).unwrap());
    assert_eq!(b"l15".as_slice(), next.as_bytes());
    assert!(next.reached_end());

    // an empty page still moves past whitespace and invalid bytes
    let page = text_page(b"  \xFF\n");
    assert!(page.is_empty());
    assert_eq!(4, page.consumed());

    let page = text_page(b"");
    assert!(page.is_empty());
    assert_eq!(0, page.consumed());
    assert!(page.reached_end());
}

#[test]
fn page_of_the_largest_rows_is_page_bytes() {
    let input = b"\xF0\x9F\xA6\x84".repeat(PAGE_ROWS * LINE_CHARS + 1);
    let page = text_page(&input);
    assert_eq!(PAGE_BYTES, page.len());
    assert_eq!(PAGE_ROWS * LINE_BYTES, page.consumed());
}

#[test]
fn queue_drops_trailing_empty_pages_only() {
    // a whitespace tail after a full page, then text: still one page per
    // stretch of text
    let input = cat(&[
        &repeat(PAGE_ROWS * LINE_CHARS, b'a'),
        &repeat(300, b' '),
        b"tail",
    ]);
    let pages = queue(&input);
    assert_eq!(2, pages.len());
    assert_eq!(b"tail".as_slice(), pages[1]);
}

#[test]
fn long_invalid_and_whitespace_runs() {
    // runs far longer than a page are skipped in one row call
    let input = cat(&[&repeat(10_000, 0xFF), &repeat(10_000, b' '), b"x"]);
    let page = text_page(&input);
    assert_eq!(b"x".as_slice(), page.as_bytes());
    assert_eq!(20_001, page.consumed());
}

/// The bounds a page keeps, from [`text_page`] directly: the length, the
/// rows, the cursor moving forward on a non-empty page, and the cursor
/// never passing the end of the text.
fn assert_page_bounds(text: &[u8]) {
    let page = text_page(text);
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    assert!(page.len() <= PAGE_BYTES);
    assert!(page.consumed() <= end);
    if !page.is_empty() {
        assert!(page.consumed() > 0);
    }
    if page.reached_end() {
        assert_eq!(end, page.consumed());
    }
    assert_eq!(1, assert_page_invariants(&[page.as_bytes().to_vec()]));
}

/// Random text from random words, weighted toward whitespace, newlines,
/// multibyte leads and continuation bytes, with valid characters of every
/// length, NBSP and runs of letters mixed in. A NUL (the end of the text)
/// is rare, so most texts run to the end of the slice. Shared with the
/// Tier 3 test.
pub(crate) fn weighted_text(words: &[u32]) -> Vec<u8> {
    const SPACES: [u8; 8] = [b' ', b'\t', b'\r', b'\n', 0x7F, 0x0B, 0x0C, 0x01];
    const LEADS: [u8; 13] = [
        0xC0, 0xC1, 0xC2, 0xC3, 0xDF, 0xE0, 0xE4, 0xED, 0xEF, 0xF0, 0xF4, 0xF5, 0xFF,
    ];
    let mut text = Vec::with_capacity(words.len() * 2);
    for &r in words {
        let pick = (r >> 8) as usize;
        match r & 0xFF {
            0 if pick.is_multiple_of(32) => text.push(0),
            0 => text.push(b'n'),
            1..=79 => text.push((r >> 8) as u8),
            80..=119 => text.push(SPACES[pick % SPACES.len()]),
            120..=159 => text.push(LEADS[pick % LEADS.len()]),
            160..=189 => text.push(0x80 | (pick % 64) as u8),
            190..=229 => text.push(b'a' + (pick % 26) as u8),
            230..=244 => {
                let c = char::from_u32(pick as u32 % 0x11_0000).unwrap_or('\u{FFFD}');
                text.extend(c.to_string().bytes());
            }
            245..=249 => text.extend_from_slice(b"\xC2\xA0"),
            _ => text.extend(std::iter::repeat_n(b'w', 1 + pick % 40)),
        }
    }
    text
}

pub(crate) fn weighted(max_words: usize) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u32>(), 0..max_words).prop_map(|w| weighted_text(&w))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// No input panics, the queue always ends, and every page keeps the
    /// bounds.
    #[test]
    fn any_bytes_keep_the_bounds(input in weighted(2000)) {
        let pages = queue(&input);
        prop_assert!(assert_page_invariants(&pages) >= 1);
        let mut cursor = 0usize;
        while cursor < input.len() {
            let rest = input.get(cursor..).unwrap();
            assert_page_bounds(rest);
            let page = text_page(rest);
            if page.consumed() == 0 {
                break;
            }
            cursor += page.consumed();
        }
    }

    /// A page that did not read the end of a prefix is the page of the
    /// whole text: the bytes after it cannot change it.
    #[test]
    fn prefix_that_was_enough(input in weighted(2000), cut in any::<prop::sample::Index>()) {
        let k = cut.index(input.len() + 1);
        let prefix = input.get(..k).unwrap();
        let short = text_page(prefix);
        if !short.reached_end() {
            let full = text_page(&input);
            prop_assert_eq!(short.as_bytes(), full.as_bytes());
            prop_assert_eq!(short.consumed(), full.consumed());
            prop_assert!(!full.reached_end());
        }
    }

    /// Fuzz-style: raw bytes of every value, long, and the helpers on
    /// every suffix.
    #[test]
    fn no_panic_on_raw_bytes(input in prop::collection::vec(any::<u8>(), 0..20000)) {
        let _ = queue(&input);
        for start in (0..input.len()).step_by(997) {
            let rest = input.get(start..).unwrap();
            let len = caption_text::utf8_char_length(rest);
            prop_assert!(len <= 4);
            let _ = caption_text::utf8_whitespace(rest, len);
            let _ = caption_text::is_newline(rest);
            let row = text_row(rest);
            prop_assert!(row.len() <= LINE_BYTES);
            prop_assert!(row.consumed() <= rest.len());
        }
    }
}
