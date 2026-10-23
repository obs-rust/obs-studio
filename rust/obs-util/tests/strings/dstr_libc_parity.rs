//! Tier 3: the Rust `dstr-libc.c` shims agree with the original C
//! `dstr-libc.c` (compiled into the oracle as `oracle_*`).
//!
//! Inputs are arbitrary: wide strings of any unit value (negative ones
//! where `wchar_t` is signed, surrogates, values past U+10FFFF, embedded
//! NULs, padding), byte strings including invalid UTF-8 and embedded NULs,
//! lengths around 0, 1 and 4096, and every starting dstr shape (unallocated,
//! spare capacity, and the capacity-0 buffer `dstr_from_mbs` leaves). The
//! return values, the resulting `(array NULL-ness, bytes through the
//! terminator, len, capacity)` of every dstr, the returned strings, and for
//! `wcsdepad` the whole buffer including the units past the new terminator,
//! are compared.
//!
//! Both sides call the C library's `towupper` / `towlower` and the same
//! `os_mbs_to_utf8_ptr` / `os_utf8_to_wcs_ptr`, so the default test runs in
//! the process's "C" locale and `utf8_locale_matches_c` repeats the
//! locale-dependent cases in a UTF-8 `LC_CTYPE`, in a child process of its
//! own (setlocale is process-wide).
//!
//! The printf family stays C. `printf_copy_matches_original` checks that the
//! copy the hybrid compiles (`util/dstr-variadic.c`, oracle `variadic_*`)
//! gives the original's results.
//!
//! No intentional differences, and no inputs are excluded: every input
//! here is defined in C.
//!
//! Mutation check (run once, not committed; each one alone fails this
//! file): in `src/dstr_libc.rs`, `compare_by` stopping one unit early under
//! `n`, `is_padding` without `'\r'`, `wstrstri` starting at position 1 or
//! not trying the terminator, `map_case` skipping the first unit, and
//! `wcsdepad` keeping the last trailing pad; in `src/ffi/dstr_libc.rs`,
//! `towlower` in place of `towupper`, `dstr_from_mbs` without its `free`,
//! `dstr_from_wcs` resizing to `len + 2`, and `dstr_to_wcs` passing a
//! length of 0; in `util/dstr-variadic.c`, either error path assuming 4094
//! bytes, and `dstr_vcatf` keeping an empty result.

use core::ffi::{CStr, c_char, c_void};
use core::ptr;

use obs_c_oracle::dstr::OracleDstr;
use obs_c_oracle::dstr_libc as c;
use obs_util::ffi::darray::{bfree, bmalloc};
use obs_util::ffi::dstr::dstr;
use obs_util::ffi::dstr_libc as r;
use obs_util::ffi::utf8::WChar;
use proptest::collection::vec;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies

/// Wide units that make matches and padding likely, plus any value.
fn unit() -> impl Strategy<Value = WChar> {
    prop_oneof![
        4 => prop::sample::select(
            b"aAbBzZ[_ \t\n\r\x0b\x0c"
                .iter()
                .map(|&b| WChar::from(b))
                .collect::<Vec<_>>()
        ),
        2 => prop::sample::select(vec![0xe9u32, 0xc9, 0xdf, 0xff, 0x178, 0x1c5, 0x10428, 0xd800, 0xdfff])
            .prop_map(|u| u as WChar),
        2 => any::<u32>().prop_map(|u| u as WChar),
        1 => Just(WChar::default()),
    ]
}

fn wide_units(max: usize) -> impl Strategy<Value = Vec<WChar>> {
    vec(unit(), 0..max)
}

/// A NUL-terminated wide buffer, or NULL.
fn opt_wide() -> impl Strategy<Value = Option<Vec<WChar>>> {
    prop_oneof![1 => Just(None), 8 => wide_units(24).prop_map(Some)]
}

/// Bytes that make UTF-8 sequences (valid and not), padding and case
/// changes likely, plus any byte.
fn byte() -> impl Strategy<Value = u8> {
    prop_oneof![
        4 => prop::sample::select(b"aAzZ 1\t\n".to_vec()),
        3 => prop::sample::select(vec![0xc3u8, 0xa9, 0x89, 0x9f, 0xe2, 0x82, 0xac, 0xf0, 0x9f, 0x98, 0x80, 0xed, 0xa0]),
        2 => prop::sample::select(vec![0xffu8, 0xfe, 0xc0, 0xc1, 0xf5, 0x80, 0xbf]),
        2 => any::<u8>(),
        1 => Just(0u8),
    ]
}

/// Bytes, with the lengths around the boundaries now and then.
fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        12 => vec(byte(), 0..48),
        1 => Just(Vec::new()),
        1 => prop::sample::select(vec![1usize, 4095, 4096, 4097])
            .prop_flat_map(|n| vec(byte(), n..=n)),
    ]
}

/// The starting shape of a dstr.
#[derive(Debug, Clone)]
enum Shape {
    /// `{NULL, 0, 0}`.
    Unallocated,
    /// The bytes and a NUL in a buffer of `len + 1 + spare` bytes.
    Bytes(Vec<u8>, usize),
    /// The bytes and a NUL with `capacity` 0, as `dstr_from_mbs` leaves it.
    Moved(Vec<u8>),
}

fn shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        1 => Just(Shape::Unallocated),
        6 => (bytes(), 0usize..40).prop_map(|(b, s)| Shape::Bytes(b, s)),
        1 => bytes().prop_map(Shape::Moved),
    ]
}

// ---------------------------------------------------------------------------
// Helpers

/// `units` and a NUL.
fn terminated(units: &[WChar]) -> Vec<WChar> {
    let mut v = units.to_vec();
    v.push(0);
    v
}

fn wide_ptr(v: &Option<Vec<WChar>>) -> *const WChar {
    v.as_ref().map_or(ptr::null(), |v| v.as_ptr())
}

/// `bytes` and a NUL.
fn cstring(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes.to_vec();
    v.push(0);
    v
}

/// One step of a chain: the operation, a byte string and a wide string.
type Op = (u8, Vec<u8>, Option<Vec<WChar>>);

/// The raw fields of a dstr, either side.
type Fields = (*mut c_char, usize, usize);

fn build(shape: &Shape) -> Fields {
    let (bytes, size, cap) = match shape {
        Shape::Unallocated => return (ptr::null_mut(), 0, 0),
        Shape::Bytes(b, spare) => (b, b.len() + 1 + spare, b.len() + 1 + spare),
        Shape::Moved(b) => (b, b.len() + 1, 0),
    };
    // SAFETY: a plain allocation of `size >= len + 1` bytes, filled below.
    let array = unsafe { bmalloc(size) } as *mut u8;
    // SAFETY: `array` holds `len + 1` bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), array, bytes.len());
        *array.add(bytes.len()) = 0;
    }
    (array as *mut c_char, bytes.len(), cap)
}

/// The same shape on both sides, in separate buffers.
fn pair(shape: &Shape) -> (dstr, OracleDstr) {
    let (a, l, cap) = build(shape);
    let (b, l2, cap2) = build(shape);
    (
        dstr {
            array: a,
            len: l,
            capacity: cap,
        },
        OracleDstr {
            array: b,
            len: l2,
            capacity: cap2,
        },
    )
}

/// What can be observed of a dstr: NULL, or the bytes through the
/// terminator, `len` and `capacity`.
fn observe(f: Fields) -> Option<(Vec<u8>, usize, usize)> {
    let (array, len, cap) = f;
    if array.is_null() {
        assert_eq!((len, cap), (0, 0), "a NULL dstr has len and capacity 0");
        return None;
    }
    // SAFETY: every dstr here holds at least `len + 1` bytes.
    let b = unsafe { core::slice::from_raw_parts(array as *const u8, len + 1) }.to_vec();
    Some((b, len, cap))
}

/// Compares and frees both dstrs.
#[track_caller]
fn check_and_free(rd: dstr, cd: OracleDstr, what: &str) -> Result<(), TestCaseError> {
    let rs = observe((rd.array, rd.len, rd.capacity));
    let cs = observe((cd.array, cd.len, cd.capacity));
    // SAFETY: both arrays are NULL or from the bmalloc family.
    unsafe {
        bfree(rd.array as *mut c_void);
        bfree(cd.array as *mut c_void);
    }
    prop_assert_eq!(rs, cs, "{}", what);
    Ok(())
}

/// Reads and frees a returned C string.
fn take_cstr(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated string from bmalloc.
    let v = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: from bmalloc.
    unsafe { bfree(p as *mut c_void) };
    Some(v)
}

/// Reads and frees a returned wide string, up to its first NUL.
fn take_wide(p: *mut WChar) -> Option<Vec<WChar>> {
    if p.is_null() {
        return None;
    }
    let mut v = Vec::new();
    // SAFETY: a NUL-terminated wide string from bmalloc.
    unsafe {
        while *p.add(v.len()) != 0 {
            v.push(*p.add(v.len()));
        }
        bfree(p as *mut c_void);
    }
    Some(v)
}

// ---------------------------------------------------------------------------
// The checks, shared by the default and the UTF-8 locale tests

fn check_compares(
    a: &Option<Vec<WChar>>,
    b: &Option<Vec<WChar>>,
    n: usize,
) -> Result<(), TestCaseError> {
    let (a, b) = (
        a.as_ref().map(|v| terminated(v)),
        b.as_ref().map(|v| terminated(v)),
    );
    let (pa, pb) = (wide_ptr(&a), wide_ptr(&b));
    // SAFETY: NULL or NUL-terminated buffers.
    unsafe {
        prop_assert_eq!(r::wstrcmpi(pa, pb), c::oracle_wstrcmpi(pa, pb));
        for n in [n, 0, 1, usize::MAX] {
            prop_assert_eq!(
                r::wstrcmp_n(pa, pb, n),
                c::oracle_wstrcmp_n(pa, pb, n),
                "n={}",
                n
            );
            prop_assert_eq!(
                r::wstrcmpi_n(pa, pb, n),
                c::oracle_wstrcmpi_n(pa, pb, n),
                "n={}",
                n
            );
        }
    }
    Ok(())
}

fn check_wstrstri(s: &Option<Vec<WChar>>, f: &Option<Vec<WChar>>) -> Result<(), TestCaseError> {
    let (s, f) = (
        s.as_ref().map(|v| terminated(v)),
        f.as_ref().map(|v| terminated(v)),
    );
    let (ps, pf) = (wide_ptr(&s), wide_ptr(&f));
    let off =
        |p: *mut WChar| (!p.is_null()).then(|| (p as usize - ps as usize) / size_of::<WChar>());
    // SAFETY: NULL or NUL-terminated buffers.
    unsafe {
        prop_assert_eq!(off(r::wstrstri(ps, pf)), off(c::oracle_wstrstri(ps, pf)));
    }
    Ok(())
}

fn check_wcsdepad(s: &[WChar], tail: &[WChar]) -> Result<(), TestCaseError> {
    let mut rb = terminated(s);
    rb.extend_from_slice(tail);
    let mut cb = rb.clone();
    // SAFETY: NUL-terminated writable buffers.
    unsafe {
        prop_assert_eq!(r::wcsdepad(rb.as_mut_ptr()), rb.as_mut_ptr());
        prop_assert_eq!(c::oracle_wcsdepad(cb.as_mut_ptr()), cb.as_mut_ptr());
    }
    prop_assert_eq!(rb, cb);
    Ok(())
}

fn check_conversions(
    start: &Shape,
    input: &[u8],
    winput: &Option<Vec<WChar>>,
) -> Result<(), TestCaseError> {
    let input = cstring(input);
    let winput = winput.as_ref().map(|v| terminated(v));
    // SAFETY: every dstr is a valid bmalloc-backed dstr built above, every
    // string argument NULL or NUL-terminated.
    unsafe {
        let (mut rd, mut cd) = pair(start);
        r::dstr_from_mbs(&mut rd, input.as_ptr().cast());
        c::oracle_dstr_from_mbs(&mut cd, input.as_ptr().cast());
        check_and_free(rd, cd, "dstr_from_mbs")?;

        let (mut rd, mut cd) = pair(start);
        r::dstr_from_mbs(&mut rd, ptr::null());
        c::oracle_dstr_from_mbs(&mut cd, ptr::null());
        check_and_free(rd, cd, "dstr_from_mbs(NULL)")?;

        let (rd, cd) = pair(start);
        prop_assert_eq!(
            take_cstr(r::dstr_to_mbs(&rd)),
            take_cstr(c::oracle_dstr_to_mbs(&cd)),
            "dstr_to_mbs"
        );
        prop_assert_eq!(
            take_wide(r::dstr_to_wcs(&rd)),
            take_wide(c::oracle_dstr_to_wcs(&cd)),
            "dstr_to_wcs"
        );
        check_and_free(rd, cd, "unchanged by dstr_to_mbs / dstr_to_wcs")?;

        let (mut rd, mut cd) = pair(start);
        r::dstr_from_wcs(&mut rd, wide_ptr(&winput));
        c::oracle_dstr_from_wcs(&mut cd, wide_ptr(&winput));
        check_and_free(rd, cd, "dstr_from_wcs")?;

        let (mut rd, mut cd) = pair(start);
        r::dstr_to_upper(&mut rd);
        c::oracle_dstr_to_upper(&mut cd);
        check_and_free(rd, cd, "dstr_to_upper")?;

        let (mut rd, mut cd) = pair(start);
        r::dstr_to_lower(&mut rd);
        c::oracle_dstr_to_lower(&mut cd);
        check_and_free(rd, cd, "dstr_to_lower")?;
    }
    Ok(())
}

/// A chain on one dstr, as callers use them: the shape a conversion leaves
/// is the start of the next.
fn check_chain(start: &Shape, ops: &[Op]) -> Result<(), TestCaseError> {
    let (mut rd, mut cd) = pair(start);
    for (op, bytes, wide) in ops {
        let bytes = cstring(bytes);
        let wide = wide.as_ref().map(|v| terminated(v));
        // SAFETY: valid dstrs; NULL or NUL-terminated arguments.
        unsafe {
            match op % 4 {
                0 => {
                    r::dstr_from_mbs(&mut rd, bytes.as_ptr().cast());
                    c::oracle_dstr_from_mbs(&mut cd, bytes.as_ptr().cast());
                }
                1 => {
                    r::dstr_from_wcs(&mut rd, wide_ptr(&wide));
                    c::oracle_dstr_from_wcs(&mut cd, wide_ptr(&wide));
                }
                2 => {
                    r::dstr_to_upper(&mut rd);
                    c::oracle_dstr_to_upper(&mut cd);
                }
                _ => {
                    r::dstr_to_lower(&mut rd);
                    c::oracle_dstr_to_lower(&mut cd);
                }
            }
        }
        prop_assert_eq!(
            observe((rd.array, rd.len, rd.capacity)),
            observe((cd.array, cd.len, cd.capacity)),
            "after op {}",
            op % 4
        );
    }
    check_and_free(rd, cd, "end of chain")
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    vec((any::<u8>(), vec(byte(), 0..24), opt_wide()), 1..8)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn compares_match_c(a in opt_wide(), b in opt_wide(), n in 0usize..32) {
        check_compares(&a, &b, n)?;
        // a prefix of the other: the compare runs into a NUL on one side
        if let (Some(a), Some(_)) = (&a, &b) {
            let p = Some(a[..a.len() / 2].to_vec());
            check_compares(&Some(a.clone()), &p, n)?;
        }
    }

    #[test]
    fn wstrstri_matches_c(s in opt_wide(), f in prop_oneof![opt_wide(), wide_units(4).prop_map(Some)]) {
        check_wstrstri(&s, &f)?;
        // a find taken from the string, so a match exists
        if let Some(s) = &s {
            let cut = s.len() / 3;
            check_wstrstri(&Some(s.clone()), &Some(s[cut..(cut + 3).min(s.len())].to_vec()))?;
        }
    }

    #[test]
    fn wcsdepad_matches_c(s in wide_units(24), tail in wide_units(6)) {
        check_wcsdepad(&s, &tail)?;
    }

    #[test]
    fn conversions_match_c(start in shape(), input in bytes(), winput in opt_wide()) {
        check_conversions(&start, &input, &winput)?;
    }

    #[test]
    fn chains_match_c(start in shape(), ops in ops()) {
        check_chain(&start, &ops)?;
    }
}

/// Every case above again with LC_CTYPE set to a UTF-8 locale, where
/// `towupper`, `towlower` and `mbstowcs` map non-ASCII text. setlocale is
/// process-wide, so this runs in a child process of its own. Skipped (and
/// says so) where the host has no UTF-8 locale.
#[test]
fn utf8_locale_matches_c() {
    if !crate::common::isolated(concat!(module_path!(), "::utf8_locale_matches_c")) {
        return;
    }
    let mut set = false;
    for name in [c"C.UTF-8", c"C.utf8", c"en_US.UTF-8", c"en_US.utf8"] {
        // SAFETY: a valid locale name; this process runs this test alone.
        if !unsafe { libc::setlocale(libc::LC_CTYPE, name.as_ptr()) }.is_null() {
            set = true;
            break;
        }
    }
    if !set {
        eprintln!("utf8_locale_matches_c: no UTF-8 locale, skipped");
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(512));
    let all = (
        opt_wide(),
        opt_wide(),
        0usize..32,
        shape(),
        bytes(),
        ops(),
        wide_units(24),
        wide_units(6),
    );
    runner
        .run(&all, |(a, b, n, start, input, ops, s, tail)| {
            check_compares(&a, &b, n)?;
            check_wstrstri(&a, &b)?;
            check_wcsdepad(&s, &tail)?;
            check_conversions(&start, &input, &b)?;
            check_chain(&start, &ops)
        })
        .unwrap();
    // The locale is in effect: U+00E9 upper-cases.
    let mut rd = dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    // SAFETY: a valid dstr and a NUL-terminated string.
    unsafe {
        obs_util::ffi::dstr::dstr_copy(&mut rd, c"\xc3\xa9".as_ptr());
        r::dstr_to_upper(&mut rd);
        assert_eq!(CStr::from_ptr(rd.array).to_bytes(), b"\xc3\x89");
        bfree(rd.array as *mut c_void);
    }
}

/// The printf family stays C; the hybrid compiles a copy of it
/// (`util/dstr-variadic.c`). The copy gives the original's dstr for the
/// same calls: formats, empty and NUL-first results, growth, long output.
#[test]
fn printf_copy_matches_original() {
    let big = vec![b'q'; 5000];
    let big = cstring(&big);
    let mut vd = OracleDstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    let mut od = OracleDstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    let state = |d: &OracleDstr| observe((d.array, d.len, d.capacity));
    macro_rules! both {
        ($f:ident, $g:ident, $($arg:expr),+) => {{
            // SAFETY: valid dstrs and arguments of the format's types.
            unsafe {
                c::$f(&mut vd, $($arg),+);
                c::$g(&mut od, $($arg),+);
            }
            assert_eq!(state(&vd), state(&od));
        }};
    }
    both!(
        variadic_dstr_printf,
        oracle_dstr_printf,
        c"%s|%d|%zu|%c|%%".as_ptr(),
        c"s".as_ptr(),
        -3,
        42usize,
        i32::from(b'z')
    );
    both!(variadic_dstr_printf, oracle_dstr_printf, c"%d".as_ptr(), 5);
    both!(
        variadic_dstr_catf,
        oracle_dstr_catf,
        c"%s".as_ptr(),
        c"0123456789012345678901234567890".as_ptr()
    );
    both!(
        variadic_dstr_catf,
        oracle_dstr_catf,
        c"%s".as_ptr(),
        c"".as_ptr()
    );
    both!(
        variadic_dstr_printf,
        oracle_dstr_printf,
        c"a%cb".as_ptr(),
        0
    );
    both!(
        variadic_dstr_catf,
        oracle_dstr_catf,
        c"%s".as_ptr(),
        c"c".as_ptr()
    );
    both!(variadic_dstr_printf, oracle_dstr_printf, c"%c".as_ptr(), 0);
    both!(
        variadic_dstr_catf,
        oracle_dstr_catf,
        c"%s".as_ptr(),
        c"".as_ptr()
    );
    both!(
        variadic_dstr_printf,
        oracle_dstr_printf,
        c"%s".as_ptr(),
        big.as_ptr()
    );
    both!(
        variadic_dstr_catf,
        oracle_dstr_catf,
        c"%s!".as_ptr(),
        big.as_ptr()
    );
    both!(
        variadic_dstr_printf,
        oracle_dstr_printf,
        c"%.*s".as_ptr(),
        3,
        c"abcdef".as_ptr()
    );
    // glibc: %ls of a non-ASCII character fails in the "C" locale, which
    // takes the C's vsnprintf error path.
    // From a fresh dstr, so the assumed 4095 bytes set the capacity.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: both arrays are from the bmalloc family.
        unsafe {
            bfree(vd.array as *mut c_void);
            bfree(od.array as *mut c_void);
        }
        (vd.array, vd.len, vd.capacity) = (ptr::null_mut(), 0, 0);
        (od.array, od.len, od.capacity) = (ptr::null_mut(), 0, 0);
        let e: [WChar; 2] = [WChar::from(0xe9u8), 0];
        both!(
            variadic_dstr_printf,
            oracle_dstr_printf,
            c"ab%lscd".as_ptr(),
            e.as_ptr()
        );
        // SAFETY: as above.
        unsafe {
            bfree(vd.array as *mut c_void);
            bfree(od.array as *mut c_void);
        }
        (vd.array, vd.len, vd.capacity) = (ptr::null_mut(), 0, 0);
        (od.array, od.len, od.capacity) = (ptr::null_mut(), 0, 0);
        both!(
            variadic_dstr_catf,
            oracle_dstr_catf,
            c"ab%lscd".as_ptr(),
            e.as_ptr()
        );
    }
    // SAFETY: both arrays are NULL or from the bmalloc family.
    unsafe {
        bfree(vd.array as *mut c_void);
        bfree(od.array as *mut c_void);
    }
}
