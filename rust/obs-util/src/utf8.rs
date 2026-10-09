//! Safe core for `utf8_to_wchar` / `wchar_to_utf8` (`libobs/util/utf8.c`,
//! non-Windows branch).
//!
//! Wide characters are 32-bit code units (`wchar_t` on Linux and macOS). The
//! core works on slices; "insize == 0 means NUL-terminated" and NULL pointer
//! handling live in the C ABI shim (`crate::ffi::utf8`).
//!
//! The C quirks are kept on purpose: overlong encodings and 5/6-byte
//! sequences are accepted, only the octets 0xc0, 0xc1, 0xf5 and 0xff and the
//! surrogate range are "forbidden", and a failed conversion may leave partial
//! output in the buffer.

/// Skip invalid input instead of failing (`UTF8_IGNORE_ERROR`).
pub const UTF8_IGNORE_ERROR: i32 = 0x01;
/// Drop U+FEFF from the output (`UTF8_SKIP_BOM`).
pub const UTF8_SKIP_BOM: i32 = 0x02;

const NXT: u8 = 0x80;
const SEQ2: u8 = 0xc0;
const SEQ3: u8 = 0xe0;
const SEQ4: u8 = 0xf0;
const SEQ5: u8 = 0xf8;
const SEQ6: u8 = 0xfc;

const BOM: u32 = 0xfeff;

/// Surrogate code points are forbidden. Negative `wchar_t` values (as `u32`
/// they are above `i32::MAX`) are not caught here, matching C.
fn wchar_forbidden(sym: u32) -> bool {
    (0xd800..=0xdfff).contains(&sym)
}

fn utf8_forbidden(octet: u8) -> bool {
    matches!(octet, 0xc0 | 0xc1 | 0xf5 | 0xff)
}

/// Returns true if `input` starts with the UTF-8 byte order mark.
pub fn has_utf8_bom(input: &[u8]) -> bool {
    input.starts_with(&[0xef, 0xbb, 0xbf])
}

/// Translates UTF-8 `input` into wide characters.
///
/// With `out == None` only the number of wide characters is returned.
/// Returns 0 on error, including `out` being `Some` but empty or too small.
pub fn utf8_to_wchar(input: &[u8], mut out: Option<&mut [u32]>, flags: i32) -> usize {
    if matches!(&out, Some(o) if o.is_empty()) {
        return 0;
    }

    let ignore = flags & UTF8_IGNORE_ERROR != 0;
    let skip_bom = flags & UTF8_SKIP_BOM != 0;

    let mut total = 0usize;
    let mut p = 0usize;
    let mut o = 0usize;

    while p < input.len() {
        let b = input[p];

        if utf8_forbidden(b) && !ignore {
            return 0;
        }

        // Number of bytes for one wide character and its high bits.
        let (n, high): (usize, u32) = if b & 0x80 == 0 {
            (1, u32::from(b))
        } else if b & 0xe0 == SEQ2 {
            (2, u32::from(b & 0x1f))
        } else if b & 0xf0 == SEQ3 {
            (3, u32::from(b & 0x0f))
        } else if b & 0xf8 == SEQ4 {
            (4, u32::from(b & 0x07))
        } else if b & 0xfc == SEQ5 {
            (5, u32::from(b & 0x03))
        } else if b & 0xfe == SEQ6 {
            (6, u32::from(b & 0x01))
        } else {
            if !ignore {
                return 0;
            }
            p += 1;
            continue;
        };

        // Does the sequence header tell the truth about the length?
        if input.len() - p < n {
            if !ignore {
                return 0;
            }
            p += 1;
            continue;
        }

        // All continuation bytes must look like 10xxxxxx.
        if input[p + 1..p + n].iter().any(|&c| c & 0xc0 != NXT) {
            if !ignore {
                return 0;
            }
            p += 1;
            continue;
        }

        total += 1;

        let Some(out) = out.as_deref_mut() else {
            p += n;
            continue;
        };

        if o >= out.len() {
            return 0; // no space left
        }

        let mut value = 0u32;
        let mut n_bits = 0;
        for &c in input[p + 1..p + n].iter().rev() {
            value |= u32::from(c & 0x3f) << n_bits;
            n_bits += 6; // 6 low bits in every byte
        }
        value |= high << n_bits;

        // C writes the value before checking it, so a skipped value stays in
        // the buffer until the next character overwrites it.
        out[o] = value;

        if wchar_forbidden(value) {
            if !ignore {
                return 0; // forbidden character
            }
            total -= 1;
        } else if value == BOM && skip_bom {
            total -= 1;
        } else {
            o += 1;
        }

        p += n;
    }

    total
}

/// Translates wide characters into UTF-8.
///
/// With `out == None` only the number of bytes is returned. Returns 0 on
/// error, including `out` being `Some` but empty or too small.
pub fn wchar_to_utf8(input: &[u32], mut out: Option<&mut [u8]>, flags: i32) -> usize {
    if matches!(&out, Some(o) if o.is_empty()) {
        return 0;
    }

    let ignore = flags & UTF8_IGNORE_ERROR != 0;
    let skip_bom = flags & UTF8_SKIP_BOM != 0;

    let mut total = 0usize;
    let mut pos = 0usize;

    for &w in input {
        if wchar_forbidden(w) {
            if !ignore {
                return 0;
            }
            continue;
        }

        if w == BOM && skip_bom {
            continue;
        }

        // `wchar_t` is signed in C: values above i32::MAX are negative.
        if w > 0x7fff_ffff {
            if !ignore {
                return 0;
            }
            continue;
        }

        let (n, lead): (usize, u8) = if w <= 0x7f {
            (1, 0)
        } else if w <= 0x7ff {
            (2, SEQ2)
        } else if w <= 0xffff {
            (3, SEQ3)
        } else if w <= 0x001f_ffff {
            (4, SEQ4)
        } else if w <= 0x03ff_ffff {
            (5, SEQ5)
        } else {
            (6, SEQ6)
        };

        total += n;

        let Some(out) = out.as_deref_mut() else {
            continue;
        };

        if out.len() - pos < n {
            return 0; // no space left
        }

        let dst = &mut out[pos..pos + n];
        if n == 1 {
            dst[0] = w as u8;
        } else {
            dst[0] = lead | (w >> (6 * (n - 1))) as u8;
            for (k, byte) in dst.iter_mut().enumerate().skip(1) {
                *byte = NXT | ((w >> (6 * (n - 1 - k))) & 0x3f) as u8;
            }
        }

        pos += n;
    }

    total
}
