//! The oracle's `util/dstr-libc.c` converts strings the way libobs does.
//!
//! dstr-libc.c's conversions call `os_utf8_to_wcs_ptr`, `os_mbs_to_utf8_ptr` and
//! `wchar_to_utf8`. The oracle used to stub all three to return 0, so every
//! conversion "failed" there while libobs performs it (#84). A port checked
//! against those stubs would have copied the failure. These tests pin known
//! conversions, so a stub cannot come back silently.

use core::ffi::c_char;
use core::ptr;

// Links the oracle's C libraries, which define the oracle_* symbols declared
// below, and the Rust bmem port that provides bmalloc/bfree.
use obs_c_oracle as _;
use obs_util as _;

#[cfg(windows)]
type WChar = u16;
#[cfg(not(windows))]
type WChar = u32;

/// `struct dstr` from `util/dstr.h`.
#[repr(C)]
struct Dstr {
    array: *mut c_char,
    len: usize,
    capacity: usize,
}

unsafe extern "C" {
    // util/dstr-libc.c, compiled into the oracle under oracle_* names.
    fn oracle_dstr_to_wcs(str: *const Dstr) -> *mut WChar;
    fn oracle_dstr_from_wcs(dst: *mut Dstr, wstr: *const WChar);
    fn oracle_dstr_from_mbs(dst: *mut Dstr, mbstr: *const c_char);
    fn oracle_dstr_to_mbs(str: *const Dstr) -> *mut c_char;
    // the Rust bmem port (obs-util)
    fn bfree(ptr: *mut core::ffi::c_void);
}

/// UTF-8 with 2-, 3- and 4-byte sequences; the last one is outside the
/// BMP, so it is a surrogate pair where `wchar_t` is 16 bits.
const TEXT: &str = "h\u{e9}llo \u{20ac} \u{1f600}";

fn wide(s: &str) -> Vec<WChar> {
    #[cfg(windows)]
    let mut v: Vec<WChar> = s.encode_utf16().collect();
    #[cfg(not(windows))]
    let mut v: Vec<WChar> = s.chars().map(u32::from).collect();
    v.push(0);
    v
}

/// Reads a NUL-terminated wide string and frees it with `bfree`.
///
/// # Safety
///
/// `p` is null or a `bmalloc`ed, NUL-terminated wide string.
unsafe fn take_wide(p: *mut WChar) -> Option<Vec<WChar>> {
    if p.is_null() {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        // SAFETY: the string is NUL-terminated, so every index up to the NUL
        // is in bounds.
        let c = unsafe { *p.add(i) };
        out.push(c);
        if c == 0 {
            break;
        }
        i += 1;
    }
    // SAFETY: `p` came from bmalloc.
    unsafe { bfree(p.cast()) };
    Some(out)
}

/// Reads the dstr's bytes and frees its array.
///
/// # Safety
///
/// `d.array` is null or a `bmalloc`ed buffer of at least `d.len` bytes.
unsafe fn take_dstr(d: Dstr) -> Vec<u8> {
    if d.array.is_null() {
        return Vec::new();
    }
    // SAFETY: the dstr holds `len` initialized bytes.
    let bytes = unsafe { core::slice::from_raw_parts(d.array.cast::<u8>(), d.len) }.to_vec();
    // SAFETY: the array came from bmalloc.
    unsafe { bfree(d.array.cast()) };
    bytes
}

fn borrowed(s: &[u8]) -> (Vec<u8>, Dstr) {
    let mut buf = s.to_vec();
    buf.push(0);
    let d = Dstr {
        array: buf.as_mut_ptr().cast(),
        len: s.len(),
        capacity: buf.len(),
    };
    (buf, d)
}

#[test]
fn dstr_to_wcs_converts_utf8() {
    let (_buf, d) = borrowed(TEXT.as_bytes());
    // SAFETY: `d` points into `_buf`, which outlives the call.
    let got = unsafe { take_wide(oracle_dstr_to_wcs(&d)) };
    assert_eq!(got, Some(wide(TEXT)));
}

#[test]
fn dstr_to_wcs_of_empty_dstr_is_null() {
    let d = Dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    // SAFETY: an empty dstr is valid input; platform.c returns NULL for it.
    assert_eq!(unsafe { take_wide(oracle_dstr_to_wcs(&d)) }, None);
}

#[test]
fn dstr_from_wcs_converts_to_utf8() {
    let w = wide(TEXT);
    let mut d = Dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    // SAFETY: `w` is NUL-terminated and `d` starts empty.
    unsafe { oracle_dstr_from_wcs(&mut d, w.as_ptr()) };
    assert_eq!(d.len, TEXT.len());
    // SAFETY: dstr_from_wcs filled `d` from bmalloc.
    assert_eq!(unsafe { take_dstr(d) }, TEXT.as_bytes());
}

/// `mbs` is the C locale here, so only ASCII round-trips the same way on
/// every platform.
#[test]
fn dstr_from_mbs_and_to_mbs_keep_ascii() {
    let mut d = Dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    // SAFETY: the literal is NUL-terminated and `d` starts empty.
    unsafe { oracle_dstr_from_mbs(&mut d, c"plain ascii".as_ptr()) };
    assert_eq!(d.len, 11);

    // SAFETY: `d` was filled by dstr_from_mbs.
    let back = unsafe { oracle_dstr_to_mbs(&d) };
    assert!(!back.is_null());
    // SAFETY: dstr_to_mbs returns a NUL-terminated bmalloc'ed string.
    let text = unsafe { core::ffi::CStr::from_ptr(back) }
        .to_bytes()
        .to_vec();
    // SAFETY: from bmalloc.
    unsafe { bfree(back.cast()) };
    assert_eq!(text, b"plain ascii");

    // SAFETY: `d` was filled by dstr_from_mbs.
    assert_eq!(unsafe { take_dstr(d) }, b"plain ascii");
}
