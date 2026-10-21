//! Safe core of `libobs/obs-output-caption-text.c.h`: the caption text
//! wrap and pagination.
//!
//! `obs_output_output_caption_text2()` (still C) turns its text into
//! pages, one [`text_page`] call at a time, until the text runs out. A page
//! is up to [`PAGE_ROWS`] rows joined by `\n`. A row is up to
//! [`LINE_CHARS`] characters: [`text_row`] skips leading whitespace and
//! invalid UTF-8, wraps at the last whitespace at or before column 32,
//! hard-wraps a longer word, ends at CR or LF, and writes every other
//! 1-byte whitespace inside the row as a space.
//!
//! The input is untrusted (obs-websocket `SendStreamCaption`, caption
//! plugins), so nothing here can panic, index out of bounds or overflow.
//! The text is the rest of a NUL-terminated C string: it ends at its first
//! NUL byte or at the end of the slice, whichever comes first.
//!
//! The output is byte for byte the C's, including what
//! `test_output_caption_text` marks "characterized, not endorsed": an
//! invalid byte is dropped, so the words on either side join; a newline is
//! skipped as leading whitespace, so blank lines give no empty rows;
//! whitespace before a newline or the end of the text stays at the end of
//! the row; with whitespace as characters 32 and 33 the row ends with a
//! space; and every C0 control byte and DEL inside a row becomes a space.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::cell::Cell;

/// Characters per row (`CAPTION_LINE_CHARS`).
pub const LINE_CHARS: usize = 32;
/// Rows per page (`CAPTION_PAGE_ROWS`).
pub const PAGE_ROWS: usize = 15;
/// The most bytes in a row: 32 characters of up to 4 bytes
/// (`CAPTION_LINE_BYTES`).
pub const LINE_BYTES: usize = 4 * LINE_CHARS;
/// The most bytes in a page: 15 full rows and 14 `\n` separators
/// (`CAPTION_PAGE_BYTES`).
pub const PAGE_BYTES: usize = PAGE_ROWS * LINE_BYTES + PAGE_ROWS - 1;

/// One row of caption text and how far it moved the cursor.
#[derive(Clone, Debug)]
pub struct Row {
    consumed: usize,
    len: usize,
    reached_end: bool,
    bytes: [u8; LINE_BYTES],
}

/// One page of caption text and how far it moved the cursor.
#[derive(Clone, Debug)]
pub struct Page {
    consumed: usize,
    len: usize,
    reached_end: bool,
    bytes: [u8; PAGE_BYTES],
}

macro_rules! text_accessors {
    ($t:ty) => {
        impl $t {
            /// How many bytes of the text the cursor moved past.
            #[must_use]
            pub fn consumed(&self) -> usize {
                self.consumed
            }

            /// The text, without a terminating NUL.
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                self.bytes.get(..self.len).unwrap_or_default()
            }

            /// The length of the text in bytes.
            #[must_use]
            pub fn len(&self) -> usize {
                self.len
            }

            /// Whether the text is empty.
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.len == 0
            }

            /// Whether the answer read the end of the slice, the NUL a C
            /// string would have there. If it did not, the bytes after
            /// the slice cannot change it, so a caller holding a prefix
            /// of a longer string knows it was enough.
            #[must_use]
            pub fn reached_end(&self) -> bool {
                self.reached_end
            }
        }
    };
}

text_accessors!(Row);
text_accessors!(Page);

/// The text as the C reads it: a byte past the slice is the NUL that ends
/// a C string. Reading one is recorded, for [`Page::reached_end`].
struct Text<'a> {
    bytes: &'a [u8],
    reached_end: Cell<bool>,
}

impl<'a> Text<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            reached_end: Cell::new(false),
        }
    }

    fn at(&self, i: usize) -> u8 {
        match self.bytes.get(i) {
            Some(&c) => c,
            None => {
                self.reached_end.set(true);
                0
            }
        }
    }

    fn char_at(&self, i: usize) -> &'a [u8] {
        self.bytes.get(i..).unwrap_or_default()
    }

    /// `caption_utf8_char_length(text + i)`. Like the C, it reads a byte
    /// only when the ones before it allow a longer character, so it never
    /// reads past the NUL.
    fn char_length(&self, i: usize) -> usize {
        let c0 = self.at(i);
        if c0 < 0x80 {
            return usize::from(c0 != 0);
        }
        let next = |k: usize| self.at(i.saturating_add(k));
        match c0 {
            0xC2..=0xDF => {
                if utf8_continuation(next(1)) {
                    2
                } else {
                    0
                }
            }
            0xE0..=0xEF => {
                let c1 = next(1);
                if !utf8_continuation(c1) || (c0 == 0xE0 && c1 < 0xA0) || (c0 == 0xED && c1 >= 0xA0)
                {
                    0
                } else if utf8_continuation(next(2)) {
                    3
                } else {
                    0
                }
            }
            0xF0..=0xF4 => {
                let c1 = next(1);
                if !utf8_continuation(c1)
                    || (c0 == 0xF0 && c1 < 0x90)
                    || (c0 == 0xF4 && c1 >= 0x90)
                    || !utf8_continuation(next(2))
                {
                    0
                } else if utf8_continuation(next(3)) {
                    4
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// `caption_is_newline(text + i)`.
    fn is_newline(&self, i: usize) -> bool {
        matches!(self.at(i), b'\r' | b'\n')
    }

    /// `caption_utf8_whitespace(text + i, bytes)`.
    fn whitespace(&self, i: usize, bytes: usize) -> bool {
        match bytes {
            1 => {
                let c = self.at(i);
                c <= b' ' || c == 0x7F
            }
            2 => self.at(i) == 0xC2 && self.at(i.saturating_add(1)) == 0xA0,
            _ => false,
        }
    }

    /// `caption_text_row(text + from)`: the row, and where the cursor ends.
    fn row(&self, from: usize) -> (Row, usize) {
        let mut row = Row {
            consumed: 0,
            len: 0,
            reached_end: false,
            bytes: [0; LINE_BYTES],
        };

        // Skip leading whitespace (newlines too) and invalid bytes.
        let mut cursor = from;
        while self.at(cursor) != 0 {
            let len = self.char_length(cursor);
            if len == 0 {
                cursor = cursor.saturating_add(1);
                continue;
            }
            if !self.whitespace(cursor, len) {
                break;
            }
            cursor = cursor.saturating_add(len);
        }

        let start = cursor;
        if self.at(start) == 0 {
            return (row, cursor);
        }

        // Find the end: a NUL or a newline, or after 32 characters the last
        // whitespace seen (which may be character 33), else character 32.
        let mut scan = start;
        let mut split = None;
        let mut hard_end = None;
        let mut chars = 0usize;
        let end = loop {
            if self.at(scan) == 0 {
                break scan;
            }
            let len = self.char_length(scan);
            if len == 0 {
                scan = scan.saturating_add(1);
                continue;
            }
            if self.is_newline(scan) {
                break scan;
            }
            if self.whitespace(scan, len) {
                split = Some(scan);
            }
            if chars == LINE_CHARS {
                // hard_end is set once chars reaches LINE_CHARS; scan is a
                // fallback that cannot be taken and still moves forward.
                break split.or(hard_end).unwrap_or(scan);
            }
            scan = scan.saturating_add(len);
            chars = chars.saturating_add(1);
            if chars == LINE_CHARS {
                hard_end = Some(scan);
            }
        };

        // Copy the valid characters before the end, 1-byte whitespace as a
        // space. At most 32 characters of at most 4 bytes: LINE_BYTES.
        let mut scan = start;
        while scan < end {
            let len = self.char_length(scan);
            if len == 0 {
                scan = scan.saturating_add(1);
                continue;
            }
            if len == 1 && self.whitespace(scan, 1) {
                push(&mut row.bytes, &mut row.len, b" ");
            } else {
                let ch = self.char_at(scan).get(..len).unwrap_or_default();
                push(&mut row.bytes, &mut row.len, ch);
            }
            scan = scan.saturating_add(len);
        }
        (row, end)
    }
}

/// Appends `src` to the `len` bytes of `buf`. The bounds of a row and a
/// page keep it inside; a byte that did not fit would be dropped, not
/// written out of bounds.
fn push(buf: &mut [u8], len: &mut usize, src: &[u8]) {
    let dst = buf.get_mut(*len..).unwrap_or_default();
    let n = src.len().min(dst.len());
    for (d, s) in dst.iter_mut().zip(src) {
        *d = *s;
    }
    *len = len.saturating_add(n);
}

/// `caption_utf8_continuation`: a UTF-8 continuation byte (`10xxxxxx`).
#[must_use]
pub fn utf8_continuation(c: u8) -> bool {
    c & 0xC0 == 0x80
}

/// `caption_utf8_char_length`: the length of the valid UTF-8 character at
/// the start of `text`, 1 for a non-NUL ASCII byte, and 0 at the end of the
/// text or for an invalid sequence (overlong, surrogate, above U+10FFFF,
/// truncated, or a byte that cannot start one). The caller drops one byte
/// on 0.
#[must_use]
pub fn utf8_char_length(text: &[u8]) -> usize {
    Text::new(text).char_length(0)
}

/// `caption_is_newline`: CR or LF.
#[must_use]
pub fn is_newline(text: &[u8]) -> bool {
    Text::new(text).is_newline(0)
}

/// `caption_utf8_whitespace`: a character of `bytes` bytes at the start of
/// `text` that counts as whitespace: an ASCII byte up to space, DEL, or
/// U+00A0.
#[must_use]
pub fn utf8_whitespace(text: &[u8], bytes: usize) -> bool {
    Text::new(text).whitespace(0, bytes)
}

/// `caption_text_row`: the next row of `text`.
///
/// Skips leading whitespace (newlines included) and invalid bytes. The row
/// ends at a NUL, at CR or LF, or once it has 32 characters: then at the
/// last whitespace seen, which may be character 33, or else after
/// character 32. The cursor stops at that end, so the whitespace a wrap
/// splits at is skipped by the next row. Invalid bytes take no column and
/// are dropped; a 1-byte whitespace character becomes a space; everything
/// else, U+00A0 included, is copied.
#[must_use]
pub fn text_row(text: &[u8]) -> Row {
    let text = Text::new(text);
    let (mut row, end) = text.row(0);
    row.consumed = end;
    row.reached_end = text.reached_end.get();
    row
}

/// `caption_text_page`: the next page of `text`: up to [`PAGE_ROWS`] rows
/// from [`text_row`], joined by `\n`. It stops early at the first empty
/// row, and the cursor ends after the last row (or after the whitespace and
/// invalid bytes the empty row skipped).
#[must_use]
pub fn text_page(text: &[u8]) -> Page {
    let text = Text::new(text);
    let mut page = Page {
        consumed: 0,
        len: 0,
        reached_end: false,
        bytes: [0; PAGE_BYTES],
    };
    let mut cursor = 0usize;

    for rows in 0..PAGE_ROWS {
        let (row, end) = text.row(cursor);
        cursor = end;
        if row.is_empty() {
            break;
        }
        if rows > 0 {
            push(&mut page.bytes, &mut page.len, b"\n");
        }
        push(&mut page.bytes, &mut page.len, row.as_bytes());
    }
    page.consumed = cursor;
    page.reached_end = text.reached_end.get();
    page
}
