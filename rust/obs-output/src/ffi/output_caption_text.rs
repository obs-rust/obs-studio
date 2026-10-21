//! C ABI for `libobs/obs-output-caption-text.c.h`.
//!
//! `obs_output_caption_text_page` is declared in `obs-internal.h` without
//! `EXPORT`, so it stays hidden (`rust-hidden.txt`):
//! `obs_output_output_caption_text2()` in `obs-output.c` calls it once per
//! page until the text runs out. It reads the rest of the NUL-terminated
//! string at `*text`, writes the page and its NUL into `page` (bytes 0 to
//! the length, the bytes the C writes), and moves `*text` past what the
//! page used, as the C does. The other functions of the C file are
//! `static`; the safe core has them as [`crate::caption_text`].
//!
//! The safe core takes a slice, and finding the whole string's length on
//! every call would read the rest of the text once per page, where the C
//! reads only what the page needs. So the shim gives the core a window of
//! the string: the bytes up to the NUL or [`FIRST_WINDOW`] bytes, whichever
//! comes first. If the page read the end of a window that is not the end
//! of the string ([`crate::caption_text::Page::reached_end`]), it doubles
//! the window and asks again; otherwise the bytes after the window cannot
//! change the page. Each call reads at most about twice what the C reads
//! (and at least the window), and never past the NUL.
//!
//! No intentional differences from the C. Where nothing can see it: a null
//! `text`, `*text` or `page` returns 0 and writes nothing, where the C
//! reads or writes through the null pointer (libobs never passes one).

use core::ffi::c_char;
use core::ptr;

use crate::caption_text;

/// The first window of the string the shim gives the core: more than a
/// page of plain text, which is about 500 bytes.
pub const FIRST_WINDOW: usize = 2048;

/// `size_t obs_output_caption_text_page(const char **text,
/// char page[CAPTION_PAGE_BYTES + 1])`.
///
/// # Safety
///
/// As the C: `text` points to a pointer to a NUL-terminated string, and
/// `page` to `CAPTION_PAGE_BYTES + 1` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_caption_text_page(
    text: *mut *const c_char,
    page: *mut c_char,
) -> usize {
    // SAFETY: the caller's contract, passed on.
    unsafe { text_page_with_window(text, page, FIRST_WINDOW) }
}

/// [`obs_output_caption_text_page`] with a given first window (at least 1
/// byte), so the Tier 3 test can make every call retry. Rust-mangled, so
/// libobs does not export it.
///
/// # Safety
///
/// As [`obs_output_caption_text_page`].
pub unsafe fn text_page_with_window(
    text: *mut *const c_char,
    page: *mut c_char,
    first_window: usize,
) -> usize {
    if text.is_null() || page.is_null() {
        return 0;
    }
    // SAFETY: `text` is a valid pointer to the cursor (caller contract).
    let start = unsafe { *text }.cast::<u8>();
    if start.is_null() {
        return 0;
    }

    // `known` bytes from `start` are non-NUL bytes of the string.
    let mut known = 0usize;
    let mut window = first_window.max(1);
    loop {
        let mut at_nul = false;
        while known < window {
            // SAFETY: the `known` bytes before this one are non-NUL bytes
            // of the string, so this one is in it too (at worst its NUL).
            if unsafe { *start.add(known) } == 0 {
                at_nul = true;
                break;
            }
            known += 1;
        }
        // SAFETY: the `known` bytes from `start` were just read: they are
        // live bytes of the caller's string, which nothing writes during
        // the call.
        let bytes = unsafe { core::slice::from_raw_parts(start, known) };
        let out = caption_text::text_page(bytes);
        if at_nul || !out.reached_end() {
            let len = out.len();
            // SAFETY: `page` has CAPTION_PAGE_BYTES + 1 bytes and `len` is
            // at most CAPTION_PAGE_BYTES (caption_text::PAGE_BYTES), so the
            // page and its NUL fit; the core's buffer is not the caller's.
            // The cursor moves within the string: `consumed` is at most
            // `known`.
            unsafe {
                ptr::copy_nonoverlapping(out.as_bytes().as_ptr(), page.cast::<u8>(), len);
                *page.add(len) = 0;
                *text = start.add(out.consumed()).cast::<c_char>();
            }
            return len;
        }
        window = window.saturating_mul(2);
    }
}
