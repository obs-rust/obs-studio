//! Safe core for `libobs/util/dstr-libc.c`: the wide-string compares and
//! search (`wstrcmpi`, `wstrcmp_n`, `wstrcmpi_n`, `wstrstri`), `wcsdepad`, and
//! the per-code-point case map behind `dstr_to_upper` / `dstr_to_lower`.
//!
//! The functions are generic over the `wchar_t` unit ([`WideUnit`]): `i32` on
//! Linux and macOS, `u16` on Windows. Units compare with that type's order,
//! as the C compares `wchar_t` values.
//!
//! The case mapping is a parameter. C calls `towupper` / `towlower`, whose
//! results depend on the C library and on the current `LC_CTYPE` locale (in
//! glibc's "C" locale `towupper(U+00E9)` is U+00E9; in "C.UTF-8" it is
//! U+00C9). The C ABI shims in [`crate::ffi::dstr_libc`] pass the C library's
//! functions, so the port maps exactly what the C maps. The mapping is one
//! unit to one unit: U+00DF stays U+00DF, it never becomes "SS".
//!
//! The rest of `dstr-libc.c` has no logic of its own to port: the
//! conversions call `os_mbs_to_utf8_ptr`, `os_utf8_to_wcs_ptr` and
//! `wchar_to_utf8` (the shims call the same functions), and the printf family
//! stays C in `util/dstr-variadic.c` because stable Rust cannot define a C
//! variadic function or take a `va_list`.

use core::cmp::Ordering;

/// A `wchar_t` unit.
pub trait WideUnit: Copy + Ord + From<u8> {}

impl<T: Copy + Ord + From<u8>> WideUnit for T {}

/// `libc_is_padding`: `' '`, `'\t'`, `'\n'` and `'\r'` only.
fn is_padding<T: WideUnit>(c: T) -> bool {
    b" \t\n\r".iter().any(|&p| c == T::from(p))
}

/// The C compare loop shared by `wstrcmpi`, `wstrcmp_n` and `wstrcmpi_n`.
///
/// `read(i)` returns unit `i` of each string (a NULL string reads as the
/// empty one). Each step compares `fold` of both units, then stops when
/// either raw unit is NUL (`*str1++ && *str2++`), or after `n` units when `n`
/// is `Some` (`--n`). `Some(0)` compares nothing. Units are read in order and
/// never past the first difference, the first NUL or `n`.
pub fn compare_by<T: WideUnit>(
    n: Option<usize>,
    fold: impl Fn(T) -> T,
    mut read: impl FnMut(usize) -> (T, T),
) -> i32 {
    if n == Some(0) {
        return 0;
    }
    let zero = T::from(0);
    let mut i = 0;
    loop {
        let (a, b) = read(i);
        match fold(a).cmp(&fold(b)) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {}
        }
        i += 1;
        if a == zero || b == zero || Some(i) == n {
            return 0;
        }
    }
}

/// `wstrstri`: the offset of the first position in `s` where `find` matches
/// case-insensitively (`fold`), or `None`.
///
/// `s` and `find` are the strings without their terminators. Like C, every
/// position up to and including the terminator is tried, and an empty `find`
/// matches at 0.
pub fn wstrstri<T: WideUnit>(s: &[T], find: &[T], fold: impl Fn(T) -> T) -> Option<usize> {
    let at = |x: &[T], i: usize| x.get(i).copied().unwrap_or_else(|| T::from(0));
    (0..=s.len())
        .find(|&i| compare_by(Some(find.len()), &fold, |k| (at(s, i + k), at(find, k))) == 0)
}

/// `wcsdepad` on `buf`, a wide string followed by its NUL (`len + 1` units).
///
/// Leading padding is removed with one move of the rest of the string and
/// its NUL to the front; trailing padding is then overwritten with NULs one
/// unit at a time. Units after those NULs keep their old values, as in C.
pub fn wcsdepad<T: WideUnit>(buf: &mut [T]) {
    let zero = T::from(0);
    // The string ends at the first NUL (or at the end of `buf`).
    let n = buf.iter().position(|&c| c == zero).unwrap_or(buf.len());
    if n == 0 {
        return;
    }
    let start = buf[..n].iter().position(|&c| !is_padding(c)).unwrap_or(n);
    let len = n - start;
    if start != 0 {
        // memmove(str, temp, (len + 1) * sizeof(wchar_t))
        buf.copy_within(start..(n + 1).min(buf.len()), 0);
    }
    if len != 0 {
        // buf[0] is not padding, so the walk stops there at the latest.
        let mut t = len - 1;
        while is_padding(buf[t]) {
            buf[t] = zero;
            if t == 0 {
                break;
            }
            t -= 1;
        }
    }
}

/// The `dstr_to_upper` / `dstr_to_lower` loop: applies `map` to every unit
/// before the first NUL (or the end of `w`).
pub fn map_case<T: WideUnit>(w: &mut [T], map: impl Fn(T) -> T) {
    let zero = T::from(0);
    w.iter_mut()
        .take_while(|c| **c != zero)
        .for_each(|c| *c = map(*c));
}
