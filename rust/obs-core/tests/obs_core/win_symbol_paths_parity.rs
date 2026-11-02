//! Tier 3: `obs_core::win_symbol_paths` against the original C.
//!
//! `obs-c-oracle` compiles the unchanged `libobs/obs-windows-symbol-paths.c.h`
//! with its three functions renamed `oracle_*`, on the C dstr oracle (the
//! original `util/dstr.c`), and it takes no Windows type, so every host builds
//! it. Every test runs the Rust shim, the C and the safe core on the same
//! input and compares the results exactly: the C's `bool` as the byte it
//! returns, and each `struct dstr` as its array's null-ness, `len`,
//! `capacity` and the `len` bytes of its array (the cut's NUL in the middle
//! included).
//!
//! - the directory over every path of one to four bytes from `a`, `A`, `/`,
//!   `\`, `:`, ` `, `.` and a non-ASCII byte, plus plugin, UNC, root and
//!   trailing-separator paths, each into a fresh dstr and into one that
//!   already holds a shorter, an equal or a longer string;
//! - the duplicate check over every list of up to three entries from NULL,
//!   `""`, ASCII letters in both cases, `é` and `É`, and paths, against
//!   each of them as the absolute path (NULL too);
//! - the join over every list of up to four of those entries, into a fresh
//!   dstr and after `"x"`;
//! - proptests over any bytes but NUL, lists of up to eight entries and
//!   prefixes.
//!
//! One intentional difference, listed here and excluded from the comparison
//! and nothing else: a NULL or empty `bin_path`. The C calls `strrchr()` on
//! the NULL array `dstr_copy()` leaves for it (it would crash), so the C is
//! not called for it; the shim frees `dir` as `dstr_copy()` does and returns
//! false, and the safe core gives no directory. Every other path is compared.
//! The shims' null `dir`, `paths` and `out` (which the C never passes) are
//! checked on their own.

use std::ffi::{CString, c_char};
use std::ptr;

use obs_c_oracle::dstr::OracleDstr;
use obs_c_oracle::win_symbol_paths as oracle;
use obs_core::ffi::win_symbol_paths as shim;
use obs_core::win_symbol_paths as rules;
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::dstr::{dstr, dstr_copy};
use proptest::prelude::*;

/// A dstr's state: whether its array is null, `len`, `capacity`, and its
/// first `len` bytes.
#[derive(Debug, PartialEq, Eq)]
struct State {
    null: bool,
    len: usize,
    capacity: usize,
    bytes: Vec<u8>,
}

fn state(array: *const c_char, len: usize, capacity: usize) -> State {
    let bytes = if array.is_null() {
        Vec::new()
    } else {
        // SAFETY: a non-null dstr array holds at least `len` bytes.
        unsafe { std::slice::from_raw_parts(array.cast::<u8>(), len) }.to_vec()
    };
    State {
        null: array.is_null(),
        len,
        capacity,
        bytes,
    }
}

fn c_string(bytes: &[u8]) -> CString {
    CString::new(bytes).expect("no NUL")
}

/// A shim dstr holding `prefix` (none when it is `None`).
fn shim_dstr(prefix: Option<&[u8]>) -> dstr {
    let mut d = dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    if let Some(prefix) = prefix {
        let p = c_string(prefix);
        // SAFETY: `d` is an empty dstr and `p` is NUL-terminated.
        unsafe { dstr_copy(&raw mut d, p.as_ptr()) };
    }
    d
}

/// A C dstr holding `prefix`, by the C dstr oracle.
fn oracle_dstr(prefix: Option<&[u8]>) -> OracleDstr {
    let mut d = OracleDstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    if let Some(prefix) = prefix {
        let p = c_string(prefix);
        // SAFETY: `d` is an empty dstr and `p` is NUL-terminated.
        unsafe { obs_c_oracle::dstr::oracle_dstr_copy(&raw mut d, p.as_ptr()) };
    }
    d
}

fn take_shim(d: dstr) -> State {
    let s = state(d.array, d.len, d.capacity);
    // SAFETY: the array is null or bmalloc'd by dstr_copy()/dstr_ncat().
    unsafe { bfree(d.array.cast()) };
    s
}

fn take_oracle(d: OracleDstr) -> State {
    let s = state(d.array, d.len, d.capacity);
    // SAFETY: the C dstr oracle allocates with the same bmalloc.
    unsafe { bfree(d.array.cast()) };
    s
}

/// The directory of `path` by the C and by the shim, into a dstr that holds
/// `prefix`, and by the safe core.
fn check_dir(path: &[u8], prefix: Option<&[u8]>) {
    assert!(
        !path.is_empty(),
        "the intentional difference is checked apart"
    );
    let p = c_string(path);

    let mut c = oracle_dstr(prefix);
    // SAFETY: `p` is NUL-terminated and `c` is a valid dstr.
    let c_ret = unsafe { oracle::oracle_obs_windows_symbol_paths_dir(p.as_ptr(), &raw mut c) };
    let c_state = take_oracle(c);

    let mut r = shim_dstr(prefix);
    // SAFETY: `p` is NUL-terminated and `r` is a valid dstr.
    let r_ret = unsafe { shim::obs_windows_symbol_paths_dir(p.as_ptr(), &raw mut r) };
    let r_state = take_shim(r);

    assert_eq!(
        u8::from(r_ret),
        c_ret,
        "dir return for {path:?} after {prefix:?}"
    );
    assert_eq!(r_state, c_state, "dir dstr for {path:?} after {prefix:?}");

    let core = rules::dir(Some(path));
    let c_dir = (c_ret != 0).then(|| {
        let end = c_state
            .bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(c_state.len);
        c_state.bytes[..end].to_vec()
    });
    assert_eq!(core, c_dir, "core dir for {path:?}");
    assert_eq!(
        rules::cut(path).copy,
        c_state
            .bytes
            .iter()
            .map(|&b| if b == 0 { b'\\' } else { b })
            .collect::<Vec<_>>()
    );
}

/// Owned C strings and the `char *` array the C reads.
struct Entries {
    _owned: Vec<Option<CString>>,
    ptrs: Vec<*mut c_char>,
}

fn entries(list: &[Option<&[u8]>]) -> Entries {
    let owned: Vec<Option<CString>> = list.iter().map(|e| e.map(c_string)).collect();
    let ptrs = owned
        .iter()
        .map(|e| {
            e.as_ref()
                .map_or(ptr::null_mut(), |s| s.as_ptr().cast_mut())
        })
        .collect();
    Entries {
        _owned: owned,
        ptrs,
    }
}

fn check_listed(list: &[Option<&[u8]>], abspath: Option<&[u8]>) {
    let e = entries(list);
    let a = abspath.map(c_string);
    let a_ptr = a.as_ref().map_or(ptr::null(), |s| s.as_ptr());
    let paths = if e.ptrs.is_empty() {
        ptr::null()
    } else {
        e.ptrs.as_ptr()
    };

    // SAFETY: `paths` holds `len` entries, each null or NUL-terminated.
    let c_ret =
        unsafe { oracle::oracle_obs_windows_symbol_paths_listed(paths, e.ptrs.len(), a_ptr) };
    // SAFETY: as above.
    let r_ret = unsafe { shim::obs_windows_symbol_paths_listed(paths, e.ptrs.len(), a_ptr) };
    assert_eq!(u8::from(r_ret), c_ret, "listed {abspath:?} in {list:?}");
    assert_eq!(
        rules::listed(list.iter().copied(), abspath),
        c_ret != 0,
        "core listed {abspath:?} in {list:?}"
    );
}

fn check_join(list: &[Option<&[u8]>], prefix: Option<&[u8]>) {
    let e = entries(list);
    let paths = if e.ptrs.is_empty() {
        ptr::null()
    } else {
        e.ptrs.as_ptr()
    };

    let mut c = oracle_dstr(prefix);
    // SAFETY: `paths` holds `len` entries and `c` is a valid dstr.
    unsafe { oracle::oracle_obs_windows_symbol_paths_join(paths, e.ptrs.len(), &raw mut c) };
    let c_state = take_oracle(c);

    let mut r = shim_dstr(prefix);
    // SAFETY: as above.
    unsafe { shim::obs_windows_symbol_paths_join(paths, e.ptrs.len(), &raw mut r) };
    let r_state = take_shim(r);

    assert_eq!(r_state, c_state, "join {list:?} after {prefix:?}");

    let mut core = prefix.unwrap_or_default().to_vec();
    core.extend(rules::join(list.iter().copied()));
    assert_eq!(core, c_state.bytes, "core join {list:?} after {prefix:?}");
}

const ALPHABET: &[u8] = b"aA/\\: .\xc3";

/// Every path of one to `max` bytes from the alphabet.
fn every_path(max: usize) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut level: Vec<Vec<u8>> = vec![Vec::new()];
    for _ in 0..max {
        let mut next = Vec::new();
        for path in &level {
            for &b in ALPHABET {
                let mut p = path.clone();
                p.push(b);
                next.push(p);
            }
        }
        out.extend(next.iter().cloned());
        level = next;
    }
    out
}

const PATHS: &[&[u8]] = &[
    b"C:\\obs\\obs-plugins\\64bit\\a.dll",
    b"C:/obs/obs-plugins/64bit/a.dll",
    b"C:/obs\\obs-plugins/64bit\\a.dll",
    b"../../obs-plugins/64bit/a.dll",
    b"obs-plugins\\a.dll",
    b"a.dll",
    b"C:a.dll",
    b"C:\\a.dll",
    b"/a.dll",
    b"C:\\obs\\",
    b"C:/obs//",
    b"\\",
    b"/",
    b"\\\\server\\share\\obs\\a.dll",
    b"//server/share/a.dll",
    b"C:\\\xc3\x9cbung/\xe6\x8f\x92\xe4\xbb\xb6\\a.dll",
    b"\xff\xfe/\x80",
];

const PREFIXES: &[Option<&[u8]>] = &[
    None,
    Some(b"x"),
    Some(b"C:\\obs\\obs-plugins\\64bit\\a.dll.longer"),
];

const ENTRIES: &[Option<&[u8]>] = &[
    None,
    Some(b""),
    Some(b"a"),
    Some(b"A"),
    Some(b"\xc3\xa9"),
    Some(b"\xc3\x89"),
    Some(b"C:\\cwd"),
    Some(b"c:\\CWD"),
];

/// Every list of up to `max` entries.
fn every_list(max: usize) -> Vec<Vec<Option<&'static [u8]>>> {
    let mut out = vec![Vec::new()];
    let mut level = vec![Vec::new()];
    for _ in 0..max {
        let mut next = Vec::new();
        for list in &level {
            for &e in ENTRIES {
                let mut l: Vec<Option<&'static [u8]>> = list.clone();
                l.push(e);
                next.push(l);
            }
        }
        out.extend(next.iter().cloned());
        level = next;
    }
    out
}

#[test]
fn parity_dir_every_short_path() {
    let paths = every_path(4);
    for (i, path) in paths.iter().enumerate() {
        if !obs_c_oracle::parity::keep(i as u64) {
            continue;
        }
        check_dir(path, None);
    }
    for path in PATHS {
        for &prefix in PREFIXES {
            check_dir(path, prefix);
        }
    }
}

#[test]
fn parity_dir_null_or_empty_is_skipped() {
    // the intentional difference: the C is not called (it would crash)
    for bin_path in [None, Some(&b""[..])] {
        let p = bin_path.map(c_string);
        let p_ptr = p.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        for &prefix in PREFIXES {
            let mut r = shim_dstr(prefix);
            // SAFETY: `p_ptr` is null or NUL-terminated; `r` is a valid dstr.
            let ret = unsafe { shim::obs_windows_symbol_paths_dir(p_ptr, &raw mut r) };
            assert!(!ret);
            // dstr_copy()'s rule for a NULL or empty source: dir is freed
            assert_eq!(take_shim(r), state(ptr::null(), 0, 0));
        }
        assert_eq!(rules::dir(bin_path), None);
    }
}

#[test]
fn parity_listed_every_short_list() {
    for (i, list) in every_list(3).iter().enumerate() {
        if !obs_c_oracle::parity::keep(i as u64) && list.len() == 3 {
            continue;
        }
        for &abspath in ENTRIES {
            check_listed(list, abspath);
        }
    }
}

#[test]
fn parity_join_every_short_list() {
    for (i, list) in every_list(4).iter().enumerate() {
        if !obs_c_oracle::parity::keep(i as u64) && list.len() == 4 {
            continue;
        }
        for &prefix in &PREFIXES[..2] {
            check_join(list, prefix);
        }
    }
}

#[test]
fn parity_shim_null_arguments() {
    let p = c_string(b"C:\\a.dll");
    // SAFETY: a null dir is refused before any read.
    assert!(!unsafe { shim::obs_windows_symbol_paths_dir(p.as_ptr(), ptr::null_mut()) });
    // SAFETY: a null array is refused before any read.
    assert!(!unsafe { shim::obs_windows_symbol_paths_listed(ptr::null(), 3, p.as_ptr()) });
    let mut out = shim_dstr(Some(b"x"));
    // SAFETY: a null array is refused before any read; `out` is untouched.
    unsafe { shim::obs_windows_symbol_paths_join(ptr::null(), 3, &raw mut out) };
    assert_eq!(
        take_shim(out),
        State {
            null: false,
            len: 1,
            capacity: 2,
            bytes: b"x".to_vec()
        }
    );
    let e = entries(&[Some(b"a")]);
    // SAFETY: a null out is refused before any write.
    unsafe { shim::obs_windows_symbol_paths_join(e.ptrs.as_ptr(), 1, ptr::null_mut()) };
}

fn path_bytes(max: usize) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            2 => Just(b'/'),
            2 => Just(b'\\'),
            1 => Just(b'a'),
            1 => Just(b'A'),
            4 => 1u8..=255,
        ],
        0..max,
    )
}

fn entry() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(Vec::new())),
        6 => path_bytes(24).prop_map(Some),
    ]
}

fn prefix() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        3 => Just(None),
        1 => prop::collection::vec(1u8..=255, 1..40).prop_map(Some),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn proptest_dir(path in path_bytes(64).prop_filter("the difference is apart", |p| !p.is_empty()), prefix in prefix()) {
        check_dir(&path, prefix.as_deref());
    }

    #[test]
    fn proptest_listed(list in prop::collection::vec(entry(), 0..8), abspath in entry(), pick in any::<prop::sample::Index>(), flip in any::<bool>()) {
        let list_refs: Vec<Option<&[u8]>> = list.iter().map(Option::as_deref).collect();
        check_listed(&list_refs, abspath.as_deref());
        // an entry itself, in another ASCII case when `flip`
        if !list.is_empty() {
            let mut again = list[pick.index(list.len())].clone();
            if flip && let Some(a) = again.as_mut() {
                    a.iter_mut().filter(|b| b.is_ascii_alphabetic()).for_each(|b| *b ^= 0x20);
            }
            check_listed(&list_refs, again.as_deref());
        }
    }

    #[test]
    fn proptest_join(list in prop::collection::vec(entry(), 0..8), prefix in prefix()) {
        let list_refs: Vec<Option<&[u8]>> = list.iter().map(Option::as_deref).collect();
        check_join(&list_refs, prefix.as_deref());
    }
}
