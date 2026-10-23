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
    let _ = (n, &fold, &mut read);
    todo!()
}

/// `wstrstri`: the offset of the first position in `s` where `find` matches
/// case-insensitively (`fold`), or `None`.
///
/// `s` and `find` are the strings without their terminators. Like C, every
/// position up to and including the terminator is tried, and an empty `find`
/// matches at 0.
pub fn wstrstri<T: WideUnit>(s: &[T], find: &[T], fold: impl Fn(T) -> T) -> Option<usize> {
    let _ = (s, find, fold);
    todo!()
}

/// `wcsdepad` on `buf`, a wide string followed by its NUL (`len + 1` units).
///
/// Leading padding is removed with one move of the rest of the string and
/// its NUL to the front; trailing padding is then overwritten with NULs one
/// unit at a time. Units after those NULs keep their old values, as in C.
pub fn wcsdepad<T: WideUnit>(buf: &mut [T]) {
    let _ = (buf, is_padding::<T>);
    todo!()
}

/// The `dstr_to_upper` / `dstr_to_lower` loop: applies `map` to every unit
/// before the first NUL (or the end of `w`).
pub fn map_case<T: WideUnit>(w: &mut [T], map: impl Fn(T) -> T) {
    let _ = (w, map);
    todo!()
}
