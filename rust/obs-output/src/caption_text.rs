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

/// `caption_utf8_continuation`: a UTF-8 continuation byte (`10xxxxxx`).
#[must_use]
pub fn utf8_continuation(c: u8) -> bool {
    let _ = c;
    todo!()
}

/// `caption_utf8_char_length`: the length of the valid UTF-8 character at
/// the start of `text`, 1 for a non-NUL ASCII byte, and 0 at the end of the
/// text or for an invalid sequence (overlong, surrogate, above U+10FFFF,
/// truncated, or a byte that cannot start one). The caller drops one byte
/// on 0.
#[must_use]
pub fn utf8_char_length(text: &[u8]) -> usize {
    let _ = text;
    todo!()
}

/// `caption_is_newline`: CR or LF.
#[must_use]
pub fn is_newline(text: &[u8]) -> bool {
    let _ = text;
    todo!()
}

/// `caption_utf8_whitespace`: a character of `bytes` bytes at the start of
/// `text` that counts as whitespace: an ASCII byte up to space, DEL, or
/// U+00A0.
#[must_use]
pub fn utf8_whitespace(text: &[u8], bytes: usize) -> bool {
    let _ = (text, bytes);
    todo!()
}

/// `caption_text_row`: the next row of `text`.
#[must_use]
pub fn text_row(text: &[u8]) -> Row {
    let _ = text;
    todo!()
}

/// `caption_text_page`: the next page of `text`.
#[must_use]
pub fn text_page(text: &[u8]) -> Page {
    let _ = text;
    todo!()
}
