//! Tier 1: `obs_core::win_symbol_paths`, the debug symbol search path rules.
//!
//! The `steps_*` cases port `test/cmocka/test_windows_symbol_paths_steps.c`
//! 1:1: its 15 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C fills a `struct dstr`; here the safe core is called directly, and the
//! dstr's array and length are its copy, its directory and the joined bytes.
//!
//! The `edge_*` cases are new: the NULL or empty binary path (the one
//! intentional difference: no directory, where the C crashes), and proptests
//! over any bytes against models written from the C's statements.

use obs_core::win_symbol_paths::{Cut, cut, dir, join, listed};
use proptest::prelude::*;

fn some(paths: &[&'static [u8]]) -> Vec<Option<&'static [u8]>> {
    paths.iter().copied().map(Some).collect()
}

fn is_listed(paths: &[Option<&[u8]>], abspath: &[u8]) -> bool {
    listed(paths.iter().copied(), Some(abspath))
}

fn joined(paths: &[Option<&[u8]>]) -> Vec<u8> {
    join(paths.iter().copied())
}

/// The directory and the copy's length, as `dir_of()` leaves the dstr.
fn dir_of(bin_path: &[u8]) -> (Option<Vec<u8>>, usize) {
    let c = cut(bin_path);
    (c.dir().map(<[u8]>::to_vec), c.copy.len())
}

#[test]
fn steps_dir_cut() {
    let path = b"C:\\obs\\obs-plugins\\64bit\\a.dll";
    let (d, len) = dir_of(path);
    assert_eq!(d.as_deref(), Some(&b"C:\\obs\\obs-plugins\\64bit"[..]));
    // characterized, not endorsed: the copy keeps the whole path's length
    assert_eq!(len, path.len());

    assert_eq!(
        dir_of(b"obs-plugins\\a.dll").0.as_deref(),
        Some(&b"obs-plugins"[..])
    );
}

#[test]
fn steps_dir_slashes() {
    let path = b"C:/obs/obs-plugins/64bit/a.dll";
    let (d, len) = dir_of(path);
    assert_eq!(d.as_deref(), Some(&b"C:\\obs\\obs-plugins\\64bit"[..]));
    assert_eq!(len, path.len());

    assert_eq!(
        dir_of(b"C:/obs\\obs-plugins/64bit\\a.dll").0.as_deref(),
        Some(&b"C:\\obs\\obs-plugins\\64bit"[..])
    );
    assert_eq!(
        dir_of(b"../../obs-plugins/64bit/a.dll").0.as_deref(),
        Some(&b"..\\..\\obs-plugins\\64bit"[..])
    );
}

#[test]
fn steps_dir_none() {
    // no separator: no directory, and the copy is kept
    assert_eq!(
        cut(b"a.dll"),
        Cut {
            copy: b"a.dll".to_vec(),
            end: None
        }
    );
    assert_eq!(cut(b"a.dll").copy.len(), 5);
    assert_eq!(
        cut(b"C:a.dll"),
        Cut {
            copy: b"C:a.dll".to_vec(),
            end: None
        }
    );
    assert_eq!(
        cut(b" "),
        Cut {
            copy: b" ".to_vec(),
            end: None
        }
    );
    for path in [&b"a.dll"[..], b"C:a.dll", b" "] {
        assert_eq!(cut(path).dir(), None);
        assert_eq!(dir(Some(path)), None);
    }
}

#[test]
fn steps_dir_trailing() {
    // characterized, not endorsed: a trailing separator cuts to the path
    // itself, without its last separator
    assert_eq!(dir_of(b"C:\\obs\\").0.as_deref(), Some(&b"C:\\obs"[..]));
    assert_eq!(dir_of(b"C:/obs//").0.as_deref(), Some(&b"C:\\obs\\"[..]));

    let (d, len) = dir_of(b"\\");
    assert_eq!(d.as_deref(), Some(&b""[..]));
    assert_eq!(len, 1);
}

#[test]
fn steps_dir_root() {
    // characterized, not endorsed: a root file gives the drive without its
    // separator
    assert_eq!(dir_of(b"C:\\a.dll").0.as_deref(), Some(&b"C:"[..]));
    assert_eq!(dir_of(b"/a.dll").0.as_deref(), Some(&b""[..]));
}

#[test]
fn steps_dir_unc() {
    assert_eq!(
        dir_of(b"\\\\server\\share\\obs\\a.dll").0.as_deref(),
        Some(&b"\\\\server\\share\\obs"[..])
    );
    assert_eq!(
        dir_of(b"//server/share/a.dll").0.as_deref(),
        Some(&b"\\\\server\\share"[..])
    );
}

#[test]
fn steps_dir_bytes() {
    // non-ASCII bytes are copied as they are; only '/' changes
    assert_eq!(
        dir_of(b"C:\\\xc3\x9cbung/\xe6\x8f\x92\xe4\xbb\xb6\\a.dll")
            .0
            .as_deref(),
        Some(&b"C:\\\xc3\x9cbung\\\xe6\x8f\x92\xe4\xbb\xb6"[..])
    );
}

#[test]
fn steps_listed_matches() {
    let paths = some(&[b"C:\\cwd", b"C:\\obs\\obs-plugins\\64bit", b"D:\\plugins"]);

    assert!(is_listed(&paths, b"C:\\cwd"));
    assert!(is_listed(&paths, b"D:\\plugins"));
    assert!(is_listed(&paths, b"C:\\obs\\obs-plugins\\64bit"));

    // ASCII case does not count
    assert!(is_listed(&paths, b"c:\\OBS\\Obs-Plugins\\64BIT"));
    assert!(is_listed(&paths, b"d:\\PLUGINS"));

    // a prefix, an extension or a separator is not a match
    assert!(!is_listed(&paths, b"C:\\cw"));
    assert!(!is_listed(&paths, b"C:\\cwd2"));
    assert!(!is_listed(&paths, b"C:\\cwd\\"));
    assert!(!is_listed(&paths, b"C:/cwd"));
    assert!(!is_listed(&paths, b""));
}

#[test]
fn steps_listed_bounds() {
    let paths = some(&[b"C:\\cwd", b"D:\\plugins"]);

    // only the first num entries
    assert!(!is_listed(&paths[..1], b"D:\\plugins"));
    assert!(is_listed(&paths[..2], b"D:\\plugins"));

    // an empty list
    assert!(!is_listed(&[], b"C:\\cwd"));
    assert!(!is_listed(&paths[..0], b"C:\\cwd"));
}

#[test]
fn steps_listed_case() {
    let paths = some(&[b"C:\\\xc3\x9cbung", b"C:\\\xc3\xa9t\xc3\xa9"]);

    assert!(is_listed(&paths, b"c:\\\xc3\x9cBUNG"));

    // characterized, not endorsed: astrcmpi() folds ASCII only, so a
    // directory that differs in a non-ASCII letter's case is listed again
    assert!(!is_listed(&paths, b"C:\\\xc3\xbcbung"));
    assert!(!is_listed(&paths, b"C:\\\xc3\x89t\xc3\x89"));
}

#[test]
fn steps_listed_empty_entries() {
    let paths: [Option<&[u8]>; 2] = [None, Some(b"")];

    // astrcmpi() reads NULL as ""
    assert!(is_listed(&paths[..1], b""));
    assert!(is_listed(&paths[1..], b""));
    assert!(!is_listed(&paths, b"C:\\cwd"));
}

#[test]
fn steps_join_counts() {
    let paths = some(&[b"C:\\cwd", b"C:\\obs\\obs-plugins\\64bit", b"D:\\plugins"]);

    // nothing to join: nothing is appended, and no search path is set
    assert_eq!(joined(&[]), b"");

    assert_eq!(joined(&paths[..1]), b"C:\\cwd");
    assert_eq!(joined(&paths[..1]).len(), 6);

    assert_eq!(
        joined(&paths),
        b"C:\\cwd;C:\\obs\\obs-plugins\\64bit;D:\\plugins"
    );
}

#[test]
fn steps_join_skips() {
    let middle_empty = [Some(&b"a"[..]), Some(b""), Some(b"c")];
    let middle_null = [Some(&b"a"[..]), None, Some(b"c")];
    let all_empty = [Some(&b""[..]), None, Some(b"")];

    assert_eq!(joined(&middle_empty), b"a;c");
    assert_eq!(joined(&middle_null), b"a;c");
    assert_eq!(joined(&all_empty), b"");
}

#[test]
fn steps_join_first_entry() {
    let first_empty = [Some(&b""[..]), Some(b"b"), Some(b"c")];
    let first_null = [None, Some(&b"b"[..])];

    // characterized, not endorsed: ';' goes before every entry but index 0,
    // so a skipped first entry leaves a leading ';'
    assert_eq!(joined(&first_empty), b";b;c");
    assert_eq!(joined(&first_null), b";b");
}

#[test]
fn steps_join_appends() {
    // the paths go after what out holds, with no separator before them
    let mut out = b"x".to_vec();
    out.extend(joined(&some(&[b"a", b"b"])));
    assert_eq!(out, b"xa;b");
    assert_eq!(out.len(), 4);
}

#[test]
fn edge_null_or_empty_path() {
    // the intentional difference: no directory, so the module is skipped
    assert_eq!(dir(None), None);
    assert_eq!(dir(Some(b"")), None);
}

#[test]
fn edge_dir_matches_cut() {
    for path in [
        &b"C:\\obs\\a.dll"[..],
        b"a.dll",
        b"\\",
        b"/",
        b"//",
        b"a/b\\c",
    ] {
        assert_eq!(dir(Some(path)), cut(path).dir().map(<[u8]>::to_vec));
    }
}

/// The C's statements: `dstr_replace(dir, "/", "\\")`, then `strrchr()`.
fn model_cut(bin_path: &[u8]) -> (Vec<u8>, Option<usize>) {
    let copy: Vec<u8> = bin_path
        .iter()
        .map(|&b| if b == b'/' { b'\\' } else { b })
        .collect();
    let end = copy.iter().rposition(|&b| b == b'\\');
    (copy, end)
}

fn path_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            Just(b'/'),
            Just(b'\\'),
            Just(b'a'),
            Just(b'A'),
            Just(b':'),
            Just(b'.'),
            1u8..=255,
        ],
        0..24,
    )
}

fn entry() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(Vec::new())),
        6 => path_bytes().prop_map(Some),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn edge_cut_any(bin_path in path_bytes().prop_filter("non-empty", |p| !p.is_empty())) {
        let (copy, end) = model_cut(&bin_path);
        let c = cut(&bin_path);
        prop_assert_eq!(&c.copy, &copy);
        prop_assert_eq!(c.end, end);
        prop_assert_eq!(c.dir(), end.map(|e| &copy[..e]));
        prop_assert_eq!(dir(Some(&bin_path)), end.map(|e| copy[..e].to_vec()));
    }

    #[test]
    fn edge_listed_any(
        entries in prop::collection::vec(entry(), 0..8),
        abspath in path_bytes(),
    ) {
        let paths: Vec<Option<&[u8]>> = entries.iter().map(Option::as_deref).collect();
        let expected = paths
            .iter()
            .any(|p| p.unwrap_or(b"").eq_ignore_ascii_case(&abspath));
        prop_assert_eq!(listed(paths.iter().copied(), Some(&abspath)), expected);
    }

    #[test]
    fn edge_listed_folds(entries in prop::collection::vec(path_bytes(), 1..6), pick in any::<prop::sample::Index>(), flips in any::<u32>()) {
        let paths: Vec<Option<&[u8]>> = entries.iter().map(|p| Some(p.as_slice())).collect();
        let mut abspath = entries[pick.index(entries.len())].clone();
        for (i, b) in abspath.iter_mut().enumerate() {
            if b.is_ascii_alphabetic() && flips.rotate_left(u32::try_from(i % 32).expect("small")) & 1 == 1 {
                *b ^= 0x20;
            }
        }
        prop_assert!(listed(paths.iter().copied(), Some(&abspath)));
    }

    #[test]
    fn edge_join_any(entries in prop::collection::vec(entry(), 0..8)) {
        let paths: Vec<Option<&[u8]>> = entries.iter().map(Option::as_deref).collect();
        let mut expected = Vec::new();
        for (i, p) in paths.iter().enumerate() {
            if let Some(p) = p.filter(|p| !p.is_empty()) {
                if i != 0 {
                    expected.push(b';');
                }
                expected.extend_from_slice(p);
            }
        }
        prop_assert_eq!(join(paths.iter().copied()), expected);
    }
}
