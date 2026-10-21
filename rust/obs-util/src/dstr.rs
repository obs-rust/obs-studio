//! Safe core for `libobs/util/dstr.c` (the functions that stay in
//! `dstr.c` after the libc-dependent ones moved to `dstr-libc.c`).
//!
//! [`Dstr`] models `struct dstr` on a `Vec<u8>` plus an explicit C-model
//! `capacity` that follows `dstr_ensure_capacity` from `dstr.h`, so the
//! observable `capacity` matches the C implementation. Arguments that are
//! `const char *` in C are byte slices read as C strings: they end at the
//! first NUL (or at the end of the slice). `Option<&[u8]>` stands for a
//! pointer that may be NULL. The raw `struct dstr` mirror lives in
//! [`crate::ffi::dstr`].
//!
//! The C bugs #46 (`dstr_insert_ch` wrote one byte past the buffer) and #47
//! (`dstr_replace` with an empty `find` never returned) were fixed in C
//! (PR #53); [`Dstr::insert_ch`] and [`Dstr::replace`] follow the fixed C
//! (an empty or NULL `find` is a no-op).
//!
//! Intentional difference from the C code: [`strlist_split`] with a NUL
//! separator treats the string as having no separator; C reads past the
//! terminator.
//!
//! Where C reads out of bounds (an index or count past the end), the safe
//! core panics instead.

use core::ffi::c_char;

use crate::darray::grow_capacity;

/// `toupper` in the C locale.
fn upper(b: u8) -> u8 {
    b.to_ascii_uppercase()
}

/// Byte `i` of a NUL-terminated string stored in `s` (NUL past the end).
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// First index `>= from` where `needle` (non-empty) starts in `hay`: a SIMD
/// `memchr` for the first byte, then compare the rest. Long strings are
/// scanned at memory speed like C `strstr`, with no per-call searcher setup
/// (#1297).
fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    let (&head, tail) = needle.split_first()?;
    let last_start = hay.len().checked_sub(needle.len())?;
    let mut i = from;
    while i <= last_start {
        i += memchr::memchr(head, &hay[i..=last_start])?;
        if &hay[i + 1..i + needle.len()] == tail {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// How `dstr_replace` changes the string held in a buffer, decided before
/// the buffer is touched (#1297: the C ABI shim edits the C buffer in place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacePlan {
    /// No match (or an empty `find`): nothing changes.
    None,
    /// Same-length or shrinking replace: done in place by
    /// [`replace_in_place`], which returns the new length. `first` is the
    /// first match.
    InPlace { first: usize },
    /// Growing replace: the string becomes `new_len` bytes; grow the buffer
    /// to hold `new_len` bytes, then call [`replace_grow`].
    Grow { new_len: usize, first: usize },
}

/// Plans `dstr_replace` of `find` with `replace` in `s`. `find` and
/// `replace` are the C strings' bytes without the terminator (callers cut
/// them with [`cstr`] once, so a long value is not rescanned).
pub fn replace_plan(s: &[u8], find: &[u8], replace: &[u8]) -> ReplacePlan {
    if find.is_empty() {
        return ReplacePlan::None;
    }
    let Some(first) = find_from(s, find, 0) else {
        return ReplacePlan::None; // no match: no copy, no allocation (#1296)
    };
    if replace.len() <= find.len() {
        return ReplacePlan::InPlace { first };
    }
    let mut count = 0usize;
    let mut at = Some(first);
    while let Some(i) = at {
        count += 1;
        at = find_from(s, find, i + find.len());
    }
    ReplacePlan::Grow {
        new_len: s.len() + count * (replace.len() - find.len()),
        first,
    }
}

/// Same-length or shrinking `dstr_replace` on `buf` (the whole string),
/// starting at the first match `first` from [`replace_plan`]; returns the
/// new length. Matches are found left to right and do not overlap, as in C.
/// `replace` must be no longer than `find` (bytes as for [`replace_plan`]).
pub fn replace_in_place(buf: &mut [u8], first: usize, find: &[u8], replace: &[u8]) -> usize {
    assert!(!find.is_empty() && replace.len() <= find.len());
    let len = buf.len();
    // `w` is where the next kept byte goes, `r` where the scan resumes;
    // `w <= r` because each replacement is no longer than its match.
    let (mut w, mut r) = (first, first);
    let mut at = Some(first);
    while let Some(i) = at {
        buf.copy_within(r..i, w);
        w += i - r;
        buf[w..w + replace.len()].copy_from_slice(replace);
        w += replace.len();
        r = i + find.len();
        at = find_from(&buf[..len], find, r);
    }
    buf.copy_within(r..len, w);
    w + len - r
}

/// Growing `dstr_replace`: `buf` is the grown string of
/// [`ReplacePlan::Grow`]'s `new_len` bytes whose first `old_len` bytes hold
/// the original string, `first` its first match. Fills `buf` with the
/// result.
pub fn replace_grow(buf: &mut [u8], old_len: usize, first: usize, find: &[u8], replace: &[u8]) {
    let new_len = buf.len();
    assert!(!find.is_empty() && replace.len() > find.len() && first < old_len);
    // Move everything after the first match's start to the end, then
    // rebuild front to back. The write position never passes the read
    // position: it trails it by the growth still to come.
    let shift = new_len - old_len;
    buf.copy_within(first..old_len, first + shift);
    let (mut w, mut r) = (first, first + shift);
    let mut at = Some(r);
    while let Some(i) = at {
        buf.copy_within(r..i, w);
        w += i - r;
        buf[w..w + replace.len()].copy_from_slice(replace);
        w += replace.len();
        r = i + find.len();
        at = find_from(&buf[..new_len], find, r);
    }
    buf.copy_within(r..new_len, w);
}

/// The C string held by `s`: everything before the first NUL.
pub fn cstr(s: &[u8]) -> &[u8] {
    match memchr::memchr(0, s) {
        Some(n) => &s[..n],
        None => s,
    }
}

/// `is_padding` from `dstr.c`.
fn is_padding(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Shared body of `astrcmpi`, `astrcmp_n` and `astrcmpi_n`. Characters are
/// compared as C `char` (signed on most platforms); `n` is `None` for the
/// unbounded `astrcmpi`.
fn compare_by(mut read: impl FnMut(usize) -> (u8, u8), n: Option<usize>, fold: bool) -> i32 {
    let mut n = n;
    if n == Some(0) {
        return 0;
    }
    let mut i = 0;
    loop {
        let (x, y) = read(i);
        let (c1, c2) = if fold {
            (upper(x) as c_char, upper(y) as c_char)
        } else {
            (x as c_char, y as c_char)
        };
        if c1 < c2 {
            return -1;
        } else if c1 > c2 {
            return 1;
        }
        if x == 0 || y == 0 {
            return 0;
        }
        if let Some(left) = n.as_mut() {
            *left -= 1;
            if *left == 0 {
                return 0;
            }
        }
        i += 1;
    }
}

fn compare(a: Option<&[u8]>, b: Option<&[u8]>, n: Option<usize>, fold: bool) -> i32 {
    let (a, b) = (a.unwrap_or(&[]), b.unwrap_or(&[]));
    compare_by(|i| (at(a, i), at(b, i)), n, fold)
}

/// Bounded compare over a lazy byte-pair reader, used by the C ABI shim so
/// pointer reads stop at the first mismatch, NUL, or n bytes as in C.
pub(crate) fn compare_n_by(n: usize, fold: bool, read: impl FnMut(usize) -> (u8, u8)) -> i32 {
    compare_by(read, Some(n), fold)
}

/// `astrcmpi`: case-insensitive compare; `None` is the empty string.
pub fn astrcmpi(a: Option<&[u8]>, b: Option<&[u8]>) -> i32 {
    compare(a, b, None, true)
}

/// `astrcmp_n`: case-sensitive compare of at most `n` characters.
pub fn astrcmp_n(a: Option<&[u8]>, b: Option<&[u8]>, n: usize) -> i32 {
    compare(a, b, Some(n), false)
}

/// `astrcmpi_n`: case-insensitive compare of at most `n` characters.
pub fn astrcmpi_n(a: Option<&[u8]>, b: Option<&[u8]>, n: usize) -> i32 {
    compare(a, b, Some(n), true)
}

/// `astrstri`: offset of the first case-insensitive match of `find` in
/// `s`, or `None` (also when either argument is NULL). An empty `find`
/// matches at offset 0.
pub fn astrstri(s: Option<&[u8]>, find: Option<&[u8]>) -> Option<usize> {
    let (s, find) = (cstr(s?), cstr(find?));
    // C also tries the terminator position; it can only match an empty
    // `find`, which already matched at 0.
    (0..=s.len()).find(|&p| compare(Some(&s[p..]), Some(find), Some(find.len()), true) == 0)
}

/// Range of `s` left after removing leading and trailing padding.
fn trim_range(s: &[u8]) -> (usize, usize) {
    let start = s.iter().position(|&b| !is_padding(b)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&b| !is_padding(b))
        .map_or(start, |i| i + 1);
    (start, end)
}

/// `strdepad` on a buffer holding a NUL-terminated string (the string ends
/// at the end of `buf` if it has no NUL). Trims padding in place and returns
/// the new string length.
pub fn strdepad(buf: &mut [u8]) -> usize {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let (start, end) = trim_range(&buf[..n]);
    let len = end - start;
    if start != 0 {
        buf.copy_within(start..end, 0);
    }
    if len < buf.len() {
        buf[len] = 0;
    }
    len
}

/// `strlist_split`: the pieces of the C string `s` between `split_ch`
/// separators; empty pieces are kept only with `include_empty`.
pub fn strlist_split(s: &[u8], split_ch: u8, include_empty: bool) -> Vec<&[u8]> {
    cstr(s)
        .split(|&b| b == split_ch)
        .filter(|p| include_empty || !p.is_empty())
        .collect()
}

/// Model of `struct dstr`: contents without the NUL terminator plus the C
/// `capacity`. `capacity == 0` is the NULL-array state. Operations keep
/// `capacity > len` whenever the array is allocated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dstr {
    bytes: Vec<u8>,
    capacity: usize,
}

/// The value that replaces the placeholder starting at `format[at]`, or `None`
/// when there is no `$1`..`$4` there or the caller passed no value for it.
///
/// The C ABI resolves placeholders in `$1`..`$4` order, so a caller that has to
/// reproduce its passes can step a format with this and know which bytes came
/// from a value.
#[inline]
pub(crate) fn placeholder_at<'a>(
    format: &[u8],
    at: usize,
    vals: [Option<&'a [u8]>; 4],
) -> Option<&'a [u8]> {
    if format.get(at) != Some(&b'$') {
        return None;
    }
    let digit = *format.get(at + 1)?;
    if !(b'1'..=b'4').contains(&digit) {
        return None;
    }
    vals[usize::from(digit - b'1')]
}

/// The length and capacity that C's `dstr_safe_printf` leaves, worked out
/// without running its passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SafePrintfPlan {
    pub(crate) len: usize,
    pub(crate) capacity: usize,
}

/// Plans a one-pass `dstr_safe_printf` of a non-empty `format` into a dstr
/// whose capacity is `capacity`, for values that hold no `$` (`format` and the
/// values already cut at their first NUL).
///
/// C runs `dstr_copy(format)` and then one `dstr_replace` per non-NULL value,
/// in `$1`..`$4` order. The copy grows the buffer to fit the format, every
/// growing replace grows it to fit its result (doubling, as
/// `dstr_ensure_capacity` does), and a shrinking replace keeps it. So the
/// final capacity depends on the order and size of those steps, not only on
/// the result (#1623). This replays the steps on the count of each
/// placeholder in the format, which is what each pass finds as long as no
/// earlier pass created a new placeholder.
///
/// Returns `None` when an earlier pass can create one: a `$` in the format
/// right before a substituted placeholder joins the start of its value
/// (`"$$1"` with `$1` = `"2"` makes a `"$2"` that the second pass replaces).
/// The caller then runs C's passes instead.
pub(crate) fn safe_printf_plan(
    format: &[u8],
    vals: [Option<&[u8]>; 4],
    capacity: usize,
) -> Option<SafePrintfPlan> {
    let mut counts = [0usize; 4];
    let mut i = 0;
    while i < format.len() {
        if placeholder_at(format, i, vals).is_some() {
            if i > 0 && format[i - 1] == b'$' {
                return None;
            }
            counts[usize::from(format[i + 1] - b'1')] += 1;
            i += 2;
        } else {
            i += 1;
        }
    }

    let ensure = |capacity: usize, size: usize| {
        if size <= capacity {
            capacity
        } else {
            grow_capacity(capacity, size)
        }
    };
    let mut len = format.len();
    let mut capacity = ensure(capacity, len + 1);
    for (val, count) in vals.into_iter().zip(counts) {
        let Some(val) = val else { continue };
        if count == 0 {
            continue;
        }
        if val.len() > 2 {
            len += (val.len() - 2) * count;
            capacity = ensure(capacity, len + 1);
        } else {
            len -= (2 - val.len()) * count;
        }
    }
    Some(SafePrintfPlan { len, capacity })
}

impl Dstr {
    /// `dstr_init`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a dstr from raw state (used by the FFI shims).
    pub fn from_parts(bytes: Vec<u8>, capacity: usize) -> Self {
        Self { bytes, capacity }
    }

    pub fn into_parts(self) -> (Vec<u8>, usize) {
        (self.bytes, self.capacity)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// The C-model `capacity`.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// `dstr_is_empty`: no array, no length, or an empty first string.
    pub fn is_empty(&self) -> bool {
        self.bytes.first().is_none_or(|&b| b == 0)
    }

    /// `dstr_free`.
    pub fn free(&mut self) {
        self.bytes = Vec::new();
        self.capacity = 0;
    }

    /// `dstr_ensure_capacity`.
    pub fn ensure_capacity(&mut self, new_size: usize) {
        if new_size <= self.capacity {
            return;
        }
        self.capacity = grow_capacity(self.capacity, new_size);
    }

    /// `dstr_reserve`: sets the capacity exactly (it may shrink).
    pub fn reserve(&mut self, capacity: usize) {
        if capacity == 0 || capacity <= self.bytes.len() {
            return;
        }
        self.capacity = capacity;
    }

    /// `dstr_resize`: new bytes are zero here (uninitialised in C).
    pub fn resize(&mut self, num: usize) {
        if num == 0 {
            self.free();
            return;
        }
        self.ensure_capacity(num + 1);
        self.bytes.resize(num, 0);
    }

    /// `dstr_copy_dstr`: `src` is the exact contents of the source.
    pub fn copy_dstr(&mut self, src: &[u8]) {
        self.free();
        if !src.is_empty() {
            self.ensure_capacity(src.len() + 1);
            self.bytes = src.to_vec();
        }
    }

    /// `dstr_cat_ch`.
    pub fn cat_ch(&mut self, ch: u8) {
        self.ensure_capacity(self.bytes.len() + 2);
        self.bytes.push(ch);
    }

    /// `dstr_cat`.
    pub fn cat(&mut self, s: &[u8]) {
        self.ncat(cstr(s));
    }

    /// `dstr_copy`: NULL or empty frees.
    pub fn copy(&mut self, s: &[u8]) {
        let s = cstr(s);
        if s.is_empty() {
            self.free();
            return;
        }
        self.ensure_capacity(s.len() + 1);
        self.bytes = s.to_vec();
    }

    /// `dstr_copy_strref` and `dstr_ncopy`: `s` is the exact `len` bytes
    /// (any NULs included). Empty frees.
    pub fn ncopy(&mut self, s: &[u8]) {
        self.free();
        if s.is_empty() {
            return;
        }
        self.bytes = s.to_vec();
        self.capacity = s.len() + 1;
    }

    /// `dstr_ncopy_dstr`: `src` is the contents of the source dstr and `len`
    /// the requested length, clamped to `src.len()`.
    pub fn ncopy_dstr(&mut self, src: &[u8], len: usize) {
        self.free();
        if len == 0 {
            return;
        }
        let newlen = len.min(src.len());
        self.bytes = src[..newlen].to_vec();
        self.capacity = newlen + 1;
    }

    /// `dstr_cat_dstr`: `s` is the exact contents of the source dstr.
    pub fn cat_dstr(&mut self, s: &[u8]) {
        if s.is_empty() {
            return;
        }
        self.ensure_capacity(self.bytes.len() + s.len() + 1);
        self.bytes.extend_from_slice(s);
    }

    /// `dstr_ncat` and `dstr_cat_strref`: `s` is the exact `len` bytes. An
    /// empty `s` or one starting with NUL is a no-op, as in C.
    pub fn ncat(&mut self, s: &[u8]) {
        if s.first().is_none_or(|&b| b == 0) {
            return;
        }
        self.ensure_capacity(self.bytes.len() + s.len() + 1);
        self.bytes.extend_from_slice(s);
    }

    /// `dstr_ncat_dstr`: `src` is the contents of the source dstr, `len`
    /// the requested length (clamped).
    pub fn ncat_dstr(&mut self, src: &[u8], len: usize) {
        if src.first().is_none_or(|&b| b == 0) || len == 0 {
            return;
        }
        let in_len = len.min(src.len());
        self.ensure_capacity(self.bytes.len() + in_len + 1);
        self.bytes.extend_from_slice(&src[..in_len]);
    }

    /// `dstr_insert`; `idx` must be at most `len` (C is undefined beyond).
    pub fn insert(&mut self, idx: usize, s: &[u8]) {
        let s = cstr(s);
        if s.is_empty() {
            return;
        }
        if idx == self.bytes.len() {
            self.cat(s);
            return;
        }
        assert!(idx <= self.bytes.len(), "dstr insert index out of range");
        self.ensure_capacity(self.bytes.len() + s.len() + 1);
        self.bytes.splice(idx..idx, s.iter().copied());
    }

    /// `dstr_insert_dstr`: `s` is the exact contents of the source dstr.
    pub fn insert_dstr(&mut self, idx: usize, s: &[u8]) {
        if s.is_empty() {
            return;
        }
        if idx == self.bytes.len() {
            self.cat_dstr(s);
            return;
        }
        assert!(idx <= self.bytes.len(), "dstr insert index out of range");
        self.ensure_capacity(self.bytes.len() + s.len() + 1);
        self.bytes.splice(idx..idx, s.iter().copied());
    }

    /// `dstr_insert_ch` (as fixed for issue #46). The capacity follows the
    /// same `ensure_capacity(len + 2)` call as C.
    pub fn insert_ch(&mut self, idx: usize, ch: u8) {
        if idx == self.bytes.len() {
            self.cat_ch(ch);
            return;
        }
        assert!(idx < self.bytes.len(), "dstr insert index out of range");
        self.ensure_capacity(self.bytes.len() + 2);
        self.bytes.insert(idx, ch);
    }

    /// `dstr_remove`: removing `len` bytes frees; otherwise the range must
    /// lie within the string.
    pub fn remove(&mut self, idx: usize, count: usize) {
        if count == 0 {
            return;
        }
        if count == self.bytes.len() {
            self.free();
            return;
        }
        let end = idx
            .checked_add(count)
            .filter(|&e| e <= self.bytes.len())
            .expect("dstr remove range out of bounds");
        self.bytes.drain(idx..end);
    }

    /// `dstr_replace`. `find` and `replace` are C strings; an empty `find`
    /// is a no-op (the C fix for issue #47).
    pub fn replace(&mut self, find: &[u8], replace: &[u8]) {
        if self.is_empty() {
            return;
        }
        let (find, replace) = (cstr(find), cstr(replace));
        match replace_plan(&self.bytes, find, replace) {
            ReplacePlan::None => {}
            ReplacePlan::InPlace { first } => {
                let n = replace_in_place(&mut self.bytes, first, find, replace);
                self.bytes.truncate(n);
            }
            ReplacePlan::Grow { new_len, first } => {
                let old_len = self.bytes.len();
                self.ensure_capacity(new_len + 1);
                self.bytes.resize(new_len, 0);
                replace_grow(&mut self.bytes, old_len, first, find, replace);
            }
        }
    }

    /// `dstr_safe_printf`: `$1`..`$4` are replaced by the non-NULL values.
    ///
    /// C copies the format and runs one `dstr_replace` per value, in order, so
    /// a later pass also substitutes a placeholder that an earlier one created:
    /// a `$2` inside the value for `$1`, or a format `$` followed by a value
    /// that starts with a digit (`"$$1"`). When neither can happen, one pass
    /// over the format gives the same bytes and avoids rescanning and
    /// rebuilding the growing buffer once per placeholder (#1296), and
    /// [`safe_printf_plan`] gives the capacity C's passes end with (#1623).
    pub fn safe_printf(&mut self, format: Option<&[u8]>, vals: [Option<&[u8]>; 4]) {
        let format = cstr(format.unwrap_or(&[]));
        // `dstr_replace` reads each value as a C string.
        let vals = vals.map(|val| val.map(cstr));
        if format.is_empty() {
            // `dstr_copy` of an empty format frees, and the replaces no-op.
            self.free();
            return;
        }

        // `memchr` is SIMD; `slice::contains` is a byte-at-a-time scan of the
        // whole value, which at 4 KB cost more than the rebuild it avoids.
        let plan = if vals
            .iter()
            .flatten()
            .any(|val| memchr::memchr(b'$', val).is_some())
        {
            None
        } else {
            safe_printf_plan(format, vals, self.capacity)
        };
        let Some(plan) = plan else {
            self.copy(format);
            for (placeholder, val) in [b"$1", b"$2", b"$3", b"$4"].into_iter().zip(vals) {
                if let Some(val) = val {
                    self.replace(placeholder, val);
                }
            }
            return;
        };

        let mut out = Vec::with_capacity(plan.len);
        let mut i = 0;
        while i < format.len() {
            if let Some(val) = placeholder_at(format, i, vals) {
                out.extend_from_slice(val);
                i += 2;
            } else {
                out.push(format[i]);
                i += 1;
            }
        }
        debug_assert_eq!(out.len(), plan.len);

        // The plan's capacity is at least the format's, so an empty result
        // from a non-empty format keeps a non-NULL buffer, as `dstr_copy` plus
        // a shrinking `dstr_replace` does (`Some([])`, not `None` in
        // `nullable_diagnostic`).
        self.capacity = plan.capacity;
        self.bytes = out;
    }

    /// `dstr_depad`: contents are read as a C string; all padding frees.
    pub fn depad(&mut self) {
        if self.capacity == 0 {
            return;
        }
        let n = self
            .bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.bytes.len());
        let (start, end) = trim_range(&self.bytes[..n]);
        if start == end {
            self.free();
        } else {
            self.bytes.truncate(end);
            self.bytes.drain(..start);
        }
    }

    /// `dstr_left` with a distinct source (`src` = its contents); `pos` must
    /// be at most `src.len()`.
    pub fn left(&mut self, src: &[u8], pos: usize) {
        assert!(pos <= src.len(), "dstr left position out of range");
        if pos == 0 {
            self.free();
            return;
        }
        self.ensure_capacity(pos + 1);
        self.bytes = src[..pos].to_vec();
    }

    /// `dstr_left` with `dst == str`. Growing past `len` is undefined in C;
    /// the new bytes are zero here.
    pub fn left_in_place(&mut self, pos: usize) {
        self.resize(pos);
    }

    /// `dstr_mid`: `src` is the contents of the source dstr.
    pub fn mid(&mut self, src: &[u8], start: usize, count: usize) {
        if count == 0 {
            self.free();
            return;
        }
        let end = start
            .checked_add(count)
            .filter(|&e| e <= src.len())
            .expect("dstr mid range out of bounds");
        self.ncopy(&src[start..end]);
    }

    /// `dstr_right`: `src` is the contents of the source dstr.
    pub fn right(&mut self, src: &[u8], pos: usize) {
        assert!(pos <= src.len(), "dstr right position out of range");
        self.ncopy(&src[pos..]);
    }
}
