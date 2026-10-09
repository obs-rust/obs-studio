//! Tier 3: the Rust `dstr.c` shims agree with the original C `dstr.c`
//! (compiled into the oracle as `oracle_*`) on arbitrary byte strings,
//! including invalid UTF-8 and embedded NULs.
//!
//! Return values and the resulting `(array NULL-ness, bytes, terminator, len,
//! capacity)` of every dstr are compared after each step of random operation
//! sequences. Both sides allocate through the shared test `bmalloc` family,
//! and every buffer is released with the same `bfree` path.
//!
//! No intentional differences: the `dstr_insert_ch` overrun (#46) and the
//! `dstr_replace` empty-`find` hang (#47) were fixed in C (PR #53), so those
//! inputs are compared like any other.
//!
//! Excluded are only inputs where C is undefined: indices or counts past
//! the end, `dstr_ncopy_dstr` from a NULL-array source with `len > 0`, a
//! NUL split character in `strlist_split`, and dstr contents with an embedded
//! NUL for `dstr_replace` (C reads them as C strings while `len` disagrees).
//! `const char *` arguments are NUL-terminated buffers of arbitrary bytes, so
//! an embedded NUL truncates the string on both sides.
//!
//! Mutation check (run by the integrator): replace one `Dstr` method body in
//! `src/dstr.rs` with `todo!()`, or change the growth rule of
//! `ensure_capacity` (for example always allocate exactly `new_size`); this
//! test and `tests/dstr.rs` must fail. Revert afterwards.

use core::ffi::{CStr, c_char};
use core::ptr;
use core::slice;

use obs_c_oracle::dstr::{self as c, OracleDstr, OracleStrref};
use obs_util::dstr::cstr;
use obs_util::ffi::darray::bfree;
use obs_util::ffi::dstr::{self as r, Strref, dstr};
use proptest::array::uniform4;
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::strategy::Union;

// ---------------------------------------------------------------------------
// Strategies

/// Bytes that make matches likely, plus arbitrary bytes (including >= 0x80).
const ALPHA: &[u8] = b"abAB _$1\t\n\r,";

fn byte() -> impl Strategy<Value = u8> {
    prop_oneof![1 => any::<u8>(), 6 => prop::sample::select(ALPHA.to_vec())]
}

/// Like `byte` but with frequent NULs, to exercise C-string truncation.
fn nul_byte() -> impl Strategy<Value = u8> {
    prop_oneof![1 => Just(0u8), 6 => byte()]
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    vec(byte(), 0..24)
}

fn short() -> impl Strategy<Value = Vec<u8>> {
    vec(byte(), 0..4)
}

fn any_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![9 => bytes(), 1 => vec(nul_byte(), 0..24)]
}

fn opt(s: impl Strategy<Value = Vec<u8>>) -> impl Strategy<Value = Option<Vec<u8>>> {
    prop::option::weighted(0.85, s)
}

// ---------------------------------------------------------------------------
// C-string and dstr plumbing

/// NUL-terminated copy of `v`.
fn nt(v: &[u8]) -> Vec<u8> {
    let mut b = v.to_vec();
    b.push(0);
    b
}

/// An optional NUL-terminated argument.
struct Cs(Option<Vec<u8>>);

impl Cs {
    fn new(v: &Option<Vec<u8>>) -> Self {
        Cs(v.as_deref().map(nt))
    }
    fn ptr(&self) -> *const c_char {
        self.0
            .as_ref()
            .map_or(ptr::null(), |b| b.as_ptr() as *const c_char)
    }
}

/// A strref over an optional NUL-terminated buffer with a bounded length.
struct Sr {
    buf: Cs,
    len: usize,
}

impl Sr {
    fn new(v: &Option<Vec<u8>>, raw: usize) -> Self {
        let len = v.as_ref().map_or(0, |v| raw % (v.len() + 1));
        Sr {
            buf: Cs::new(v),
            len,
        }
    }
    fn r(&self) -> Strref {
        Strref {
            array: self.buf.ptr(),
            len: self.len,
        }
    }
    fn c(&self) -> OracleStrref {
        OracleStrref {
            array: self.buf.ptr(),
            len: self.len,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Snap {
    null: bool,
    bytes: Vec<u8>,
    terminator: Option<u8>,
    capacity: usize,
}

/// # Safety
///
/// `array` must be null or valid for `len` bytes (and `len + 1` when
/// `capacity > len`).
unsafe fn snap(array: *const c_char, len: usize, capacity: usize) -> Snap {
    if array.is_null() {
        return Snap {
            null: true,
            bytes: Vec::new(),
            terminator: None,
            capacity,
        };
    }
    let n = if capacity > len { len + 1 } else { len };
    // SAFETY: per the contract.
    let all = unsafe { slice::from_raw_parts(array as *const u8, n) };
    Snap {
        null: false,
        bytes: all[..len].to_vec(),
        terminator: all.get(len).copied(),
        capacity,
    }
}

/// A Rust-side and a C-side dstr kept in lockstep.
struct Pair {
    r: dstr,
    c: OracleDstr,
}

impl Pair {
    fn new() -> Self {
        Pair {
            r: dstr {
                array: ptr::null_mut(),
                len: 0,
                capacity: 0,
            },
            c: OracleDstr {
                array: ptr::null_mut(),
                len: 0,
                capacity: 0,
            },
        }
    }

    /// Frees both dstrs (via `Drop`) and starts over like `dstr_init`.
    fn reset(&mut self) {
        *self = Pair::new();
    }

    fn snaps(&self) -> (Snap, Snap) {
        // SAFETY: both dstrs were built by the ported/oracle functions, which
        // keep `array` valid for `capacity` bytes.
        unsafe {
            (
                snap(self.r.array, self.r.len, self.r.capacity),
                snap(self.c.array, self.c.len, self.c.capacity),
            )
        }
    }

    fn check(&self) -> Result<(), TestCaseError> {
        let (a, b) = self.snaps();
        prop_assert_eq!(a, b);
        Ok(())
    }

    fn rust_bytes(&self) -> &[u8] {
        if self.r.array.is_null() || self.r.len == 0 {
            &[]
        } else {
            // SAFETY: a non-null array holds `len` bytes.
            unsafe { slice::from_raw_parts(self.r.array as *const u8, self.r.len) }
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: both arrays are null or from the shared test bmalloc.
        unsafe {
            bfree(self.r.array as *mut _);
            bfree(self.c.array as *mut _);
        }
    }
}

// ---------------------------------------------------------------------------
// Operation sequences on a destination and a source dstr

#[derive(Debug, Clone)]
enum Op {
    SetSrc(Vec<u8>),
    InitCopyStrref(Option<Vec<u8>>, usize),
    Copy(Option<Vec<u8>>),
    CopyStrref(Option<Vec<u8>>, usize),
    Ncopy(Vec<u8>, usize),
    NcopyDstr(usize),
    CatDstr,
    CatStrref(Option<Vec<u8>>, usize),
    Ncat(Option<Vec<u8>>, usize),
    NcatDstr(usize),
    Insert(usize, Option<Vec<u8>>),
    InsertDstr(usize),
    InsertCh(usize, u8),
    Remove(usize, usize),
    SafePrintf(Option<Vec<u8>>, [Option<Vec<u8>>; 4]),
    Replace(Vec<u8>, Option<Vec<u8>>),
    Depad,
    Left(usize),
    LeftInPlace(usize),
    Mid(usize, usize),
    Right(usize),
}

fn op() -> impl Strategy<Value = Op> {
    let n = || any::<usize>();
    Union::new(vec![
        any_bytes().prop_map(Op::SetSrc).boxed(),
        (opt(any_bytes()), n())
            .prop_map(|(v, l)| Op::InitCopyStrref(v, l))
            .boxed(),
        opt(any_bytes()).prop_map(Op::Copy).boxed(),
        (opt(any_bytes()), n())
            .prop_map(|(v, l)| Op::CopyStrref(v, l))
            .boxed(),
        (any_bytes(), n())
            .prop_map(|(v, l)| Op::Ncopy(v, l))
            .boxed(),
        n().prop_map(Op::NcopyDstr).boxed(),
        Just(Op::CatDstr).boxed(),
        (opt(any_bytes()), n())
            .prop_map(|(v, l)| Op::CatStrref(v, l))
            .boxed(),
        (opt(any_bytes()), n())
            .prop_map(|(v, l)| Op::Ncat(v, l))
            .boxed(),
        n().prop_map(Op::NcatDstr).boxed(),
        (n(), opt(any_bytes()))
            .prop_map(|(i, v)| Op::Insert(i, v))
            .boxed(),
        n().prop_map(Op::InsertDstr).boxed(),
        (n(), any::<u8>())
            .prop_map(|(i, ch)| Op::InsertCh(i, ch))
            .boxed(),
        (n(), n()).prop_map(|(i, k)| Op::Remove(i, k)).boxed(),
        (opt(bytes()), uniform4(opt(short())))
            .prop_map(|(f, v)| Op::SafePrintf(f, v))
            .boxed(),
        (short(), opt(bytes()))
            .prop_map(|(f, v)| Op::Replace(f, v))
            .boxed(),
        Just(Op::Depad).boxed(),
        n().prop_map(Op::Left).boxed(),
        n().prop_map(Op::LeftInPlace).boxed(),
        (n(), n()).prop_map(|(a, b)| Op::Mid(a, b)).boxed(),
        n().prop_map(Op::Right).boxed(),
    ])
}

/// Runs `op` on both sides, skipping it when its inputs are excluded.
fn apply(op: &Op, d: &mut Pair, s: &mut Pair) {
    let ld = d.r.len;
    let ls = s.r.len;
    let s_null = s.r.array.is_null();
    // SAFETY: both pairs hold valid dstrs of identical state that are only
    // changed through these functions; strings are NUL-terminated buffers
    // that outlive the calls; indices and counts are reduced to ranges C
    // defines; `src` and `dst` are distinct unless the operation is the
    // in-place `dstr_left`.
    unsafe {
        match op {
            Op::SetSrc(v) => {
                let b = nt(v);
                r::dstr_ncopy(&mut s.r, b.as_ptr() as *const c_char, v.len());
                c::oracle_dstr_ncopy(&mut s.c, b.as_ptr() as *const c_char, v.len());
            }
            Op::InitCopyStrref(v, raw) => {
                d.reset();
                let sr = Sr::new(v, *raw);
                let (a, b) = (sr.r(), sr.c());
                r::dstr_init_copy_strref(&mut d.r, &a);
                c::oracle_dstr_init_copy_strref(&mut d.c, &b);
            }
            Op::Copy(v) => {
                let cs = Cs::new(v);
                r::dstr_copy(&mut d.r, cs.ptr());
                c::oracle_dstr_copy(&mut d.c, cs.ptr());
            }
            Op::CopyStrref(v, raw) => {
                let sr = Sr::new(v, *raw);
                let (a, b) = (sr.r(), sr.c());
                r::dstr_copy_strref(&mut d.r, &a);
                c::oracle_dstr_copy_strref(&mut d.c, &b);
            }
            Op::Ncopy(v, raw) => {
                let b = nt(v);
                let len = raw % (v.len() + 1);
                r::dstr_ncopy(&mut d.r, b.as_ptr() as *const c_char, len);
                c::oracle_dstr_ncopy(&mut d.c, b.as_ptr() as *const c_char, len);
            }
            Op::NcopyDstr(raw) => {
                let len = raw % (ls + 3);
                if s_null && len > 0 {
                    return; // C copies from a NULL array
                }
                r::dstr_ncopy_dstr(&mut d.r, &s.r, len);
                c::oracle_dstr_ncopy_dstr(&mut d.c, &s.c, len);
            }
            Op::CatDstr => {
                r::dstr_cat_dstr(&mut d.r, &s.r);
                c::oracle_dstr_cat_dstr(&mut d.c, &s.c);
            }
            Op::CatStrref(v, raw) => {
                let sr = Sr::new(v, *raw);
                let (a, b) = (sr.r(), sr.c());
                r::dstr_cat_strref(&mut d.r, &a);
                c::oracle_dstr_cat_strref(&mut d.c, &b);
            }
            Op::Ncat(v, raw) => {
                let cs = Cs::new(v);
                let len = raw % v.as_ref().map_or(8, |v| v.len() + 1);
                r::dstr_ncat(&mut d.r, cs.ptr(), len);
                c::oracle_dstr_ncat(&mut d.c, cs.ptr(), len);
            }
            Op::NcatDstr(raw) => {
                let len = raw % (ls + 3);
                r::dstr_ncat_dstr(&mut d.r, &s.r, len);
                c::oracle_dstr_ncat_dstr(&mut d.c, &s.c, len);
            }
            Op::Insert(raw, v) => {
                let cs = Cs::new(v);
                let idx = raw % (ld + 1);
                r::dstr_insert(&mut d.r, idx, cs.ptr());
                c::oracle_dstr_insert(&mut d.c, idx, cs.ptr());
            }
            Op::InsertDstr(raw) => {
                let idx = raw % (ld + 1);
                r::dstr_insert_dstr(&mut d.r, idx, &s.r);
                c::oracle_dstr_insert_dstr(&mut d.c, idx, &s.c);
            }
            Op::InsertCh(raw, ch) => {
                let idx = raw % (ld + 1);
                r::dstr_insert_ch(&mut d.r, idx, *ch as c_char);
                c::oracle_dstr_insert_ch(&mut d.c, idx, *ch as c_char);
            }
            Op::Remove(a, b) => {
                let (idx, count) = (a % (ld + 1), b % (ld + 2));
                if !(count == 0 || count == ld || idx + count <= ld) {
                    return; // C moves memory out of range
                }
                r::dstr_remove(&mut d.r, idx, count);
                c::oracle_dstr_remove(&mut d.c, idx, count);
            }
            Op::SafePrintf(format, vals) => {
                let f = Cs::new(format);
                let v: Vec<Cs> = vals.iter().map(Cs::new).collect();
                r::dstr_safe_printf(
                    &mut d.r,
                    f.ptr(),
                    v[0].ptr(),
                    v[1].ptr(),
                    v[2].ptr(),
                    v[3].ptr(),
                );
                c::oracle_dstr_safe_printf(
                    &mut d.c,
                    f.ptr(),
                    v[0].ptr(),
                    v[1].ptr(),
                    v[2].ptr(),
                    v[3].ptr(),
                );
            }
            Op::Replace(find, replace) => {
                if d.rust_bytes().contains(&0) {
                    return; // C reads the content as a C string, `len` disagrees
                }
                let f = nt(find);
                let rep = Cs::new(replace);
                r::dstr_replace(&mut d.r, f.as_ptr() as *const c_char, rep.ptr());
                c::oracle_dstr_replace(&mut d.c, f.as_ptr() as *const c_char, rep.ptr());
            }
            Op::Depad => {
                r::dstr_depad(&mut d.r);
                c::oracle_dstr_depad(&mut d.c);
            }
            Op::Left(raw) => {
                let pos = raw % (ls + 1);
                r::dstr_left(&mut d.r, &s.r, pos);
                c::oracle_dstr_left(&mut d.c, &s.c, pos);
            }
            Op::LeftInPlace(raw) => {
                let pos = raw % (ld + 1);
                let (pr, pc) = (&raw mut d.r, &raw mut d.c);
                r::dstr_left(pr, pr, pos);
                c::oracle_dstr_left(pc, pc, pos);
            }
            Op::Mid(a, b) => {
                let start = a % (ls + 1);
                let count = b % (ls - start + 1);
                r::dstr_mid(&mut d.r, &s.r, start, count);
                c::oracle_dstr_mid(&mut d.c, &s.c, start, count);
            }
            Op::Right(raw) => {
                let pos = raw % (ls + 1);
                r::dstr_right(&mut d.r, &s.r, pos);
                c::oracle_dstr_right(&mut d.c, &s.c, pos);
            }
        }
    }
}

proptest! {
    #[test]
    fn dstr_ops_match_c(ops in vec(op(), 0..24)) {
        let mut d = Pair::new();
        let mut s = Pair::new();
        for op in &ops {
            apply(op, &mut d, &mut s);
            d.check()?;
            s.check()?;
        }
    }
}

// ---------------------------------------------------------------------------
// Stateless functions

fn swap_case(b: u8) -> u8 {
    if b.is_ascii_lowercase() {
        b.to_ascii_uppercase()
    } else {
        b.to_ascii_lowercase()
    }
}

/// A string and a case-flipped variant of it (plus a random tail), or two
/// unrelated optional strings.
fn str_pair() -> impl Strategy<Value = (Option<Vec<u8>>, Option<Vec<u8>>)> {
    prop_oneof![
        (opt(any_bytes()), opt(any_bytes())),
        (any_bytes(), vec(any::<bool>(), 24), short()).prop_map(|(a, flips, tail)| {
            let b: Vec<u8> = a
                .iter()
                .zip(flips.iter().cycle())
                .map(|(&ch, &f)| if f { swap_case(ch) } else { ch })
                .chain(tail)
                .collect();
            (Some(a), Some(b))
        }),
    ]
}

fn count() -> impl Strategy<Value = usize> {
    prop_oneof![0usize..20, Just(usize::MAX)]
}

proptest! {
    #[test]
    fn compare_functions_match_c((a, b) in str_pair(), n in count()) {
        let (ca, cb) = (Cs::new(&a), Cs::new(&b));
        // SAFETY: the arguments are NULL or NUL-terminated buffers.
        unsafe {
            prop_assert_eq!(
                r::astrcmpi(ca.ptr(), cb.ptr()),
                c::oracle_astrcmpi(ca.ptr(), cb.ptr())
            );
            prop_assert_eq!(
                r::astrcmp_n(ca.ptr(), cb.ptr(), n),
                c::oracle_astrcmp_n(ca.ptr(), cb.ptr(), n)
            );
            prop_assert_eq!(
                r::astrcmpi_n(ca.ptr(), cb.ptr(), n),
                c::oracle_astrcmpi_n(ca.ptr(), cb.ptr(), n)
            );
        }
    }

    #[test]
    fn astrstri_matches_c(
        hay in opt(any_bytes()),
        from in any::<usize>(),
        take in 0usize..6,
        flips in vec(any::<bool>(), 6),
        independent in opt(short()),
        use_slice in any::<bool>(),
    ) {
        let needle = match (&hay, use_slice) {
            (Some(h), true) => {
                let start = from % (h.len() + 1);
                let end = (start + take).min(h.len());
                Some(
                    h[start..end]
                        .iter()
                        .zip(flips.iter().cycle())
                        .map(|(&ch, &f)| if f { swap_case(ch) } else { ch })
                        .collect(),
                )
            }
            _ => independent,
        };
        let (ch, cn) = (Cs::new(&hay), Cs::new(&needle));
        // SAFETY: the arguments are NULL or NUL-terminated buffers; a
        // non-NULL result points into the haystack buffer.
        unsafe {
            let a = r::astrstri(ch.ptr(), cn.ptr());
            let b = c::oracle_astrstri(ch.ptr(), cn.ptr());
            prop_assert_eq!(a.is_null(), b.is_null());
            if !a.is_null() {
                prop_assert_eq!(a as usize - ch.ptr() as usize, b as usize - ch.ptr() as usize);
            }
        }
    }

    #[test]
    fn strdepad_matches_c(input in prop::option::of(vec(prop_oneof![nul_byte(), Just(b' '), Just(b'\t'), Just(b'\n'), Just(b'\r')], 0..16))) {
        let mut a = input.as_deref().map(nt);
        let mut b = a.clone();
        let pa = a.as_mut().map_or(ptr::null_mut(), |v| v.as_mut_ptr() as *mut c_char);
        let pb = b.as_mut().map_or(ptr::null_mut(), |v| v.as_mut_ptr() as *mut c_char);
        // SAFETY: the pointers are NULL or writable NUL-terminated buffers.
        let (ra, rb) = unsafe { (r::strdepad(pa), c::oracle_strdepad(pb)) };
        // the same pointer is returned
        prop_assert_eq!(ra, pa);
        prop_assert_eq!(rb, pb);
        if let (Some(a), Some(b)) = (a, b) {
            prop_assert_eq!(cstr(&a), cstr(&b));
        }
    }

    #[test]
    fn strlist_split_matches_c(
        input in opt(prop_oneof![bytes(), vec(nul_byte(), 0..16)]),
        sep in prop_oneof![Just(b','), Just(b'a'), 1u8..=255],
        include_empty in any::<bool>(),
    ) {
        let cs = Cs::new(&input);
        // SAFETY: the input is NULL or NUL-terminated; the lists are read
        // until their NULL entry and freed once.
        unsafe {
            let a = r::strlist_split(cs.ptr(), sep as c_char, include_empty);
            let b = c::oracle_strlist_split(cs.ptr(), sep as c_char, include_empty);
            let (la, lb) = (read_list(a), read_list(b));
            r::strlist_free(a);
            c::oracle_strlist_free(b);
            prop_assert_eq!(la, lb);
        }
    }
}

/// Entries of a `strlist_split` result as `(offset from the table, string)`;
/// the offsets pin the single-block layout.
///
/// # Safety
///
/// `list` must be NULL or a valid NULL-terminated pointer table.
unsafe fn read_list(list: *mut *mut c_char) -> Option<Vec<(usize, Vec<u8>)>> {
    if list.is_null() {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        // SAFETY: the table is NULL terminated.
        let p = unsafe { *list.add(i) };
        if p.is_null() {
            return Some(out);
        }
        // SAFETY: entries are NUL-terminated strings.
        let s = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
        out.push((p as usize - list as usize, s));
        i += 1;
    }
}
