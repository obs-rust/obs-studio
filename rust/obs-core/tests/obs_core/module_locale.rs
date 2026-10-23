//! Tier 1: the safe core of `libobs/obs-module-locale.c.h`.
//!
//! The first test mirrors the `module_locale` case of
//! `test/cmocka/test_core_module_load.c` one to one, in the same order,
//! with the same files and the same expected text. [`load`] stands for
//! `obs_module_load_locale()`: it looks the [`default_file`] up with
//! `module_paths::find_file` (`obs_find_module_file()`), stops when that is
//! missing or the locale [`is_default`], and adds the [`locale_file`] when
//! it exists. The root is "/R" where the C test writes "R", and the
//! working directory of the empty data path case is "/R/loc", as there.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#1876) and
//! kept as the C has them. The tests after the first pin the edges: NULL
//! against empty, case, bytes outside ASCII, '/' and ".." in a locale, and
//! proptests over any bytes.

use std::cell::RefCell;
use std::collections::BTreeMap;

use obs_core::module_locale::{
    DEFAULT_PREFIX, LOCALE_EXT, LOCALE_PREFIX, default_file, is_default, locale_file,
};
use obs_core::module_paths::{GlobEntry, ModuleFs, find_file};
use proptest::prelude::*;

const EN: (&str, Option<&str>) = ("en A", Some("en B"));
const DE: (&str, Option<&str>) = ("de A", None);
const EN_CASE: (&str, Option<&str>) = ("EN-us A", Some("EN-us B"));

/// The working directory of the empty data path case.
const CWD: &str = "/R/loc";

/// A file system of locale files, as a real one resolves paths: relative
/// to [`CWD`], with repeated '/' as one. Logs every `file_exists()`.
#[derive(Default)]
struct FakeFs {
    files: BTreeMap<String, (&'static str, Option<&'static str>)>,
    asked: RefCell<Vec<String>>,
}

fn resolve(path: &str) -> String {
    let full = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("{CWD}/{path}")
    };
    let mut out = String::new();
    for c in full.chars() {
        if !(c == '/' && out.ends_with('/')) {
            out.push(c);
        }
    }
    out
}

impl FakeFs {
    /// The files of the C test.
    fn module_locale() -> Self {
        let mut fs = Self::default();
        fs.files.insert("/R/loc/locale/en-US.ini".into(), EN);
        fs.files.insert("/R/loc/locale/de-DE.ini".into(), DE);
        fs.files.insert("/R/loc/locale/EN-us.ini".into(), EN_CASE);
        fs.files.insert("/R/nodef/locale/de-DE.ini".into(), DE);
        fs
    }

    fn text(&self, path: &[u8]) -> (&'static str, Option<&'static str>) {
        self.files[&resolve(&String::from_utf8_lossy(path))]
    }
}

impl ModuleFs for FakeFs {
    fn file_exists(&self, path: &[u8]) -> bool {
        let path = String::from_utf8_lossy(path).into_owned();
        let found = self.files.contains_key(&resolve(&path));
        self.asked.borrow_mut().push(path);
        found
    }

    fn glob(&self, _pattern: &[u8]) -> Option<Vec<GlobEntry>> {
        panic!("the locale files are not globbed");
    }
}

/// `obs_module_load_locale()` for a module with the data path `data`
/// (`module` false: a NULL module): the text of A and B, `None` for a NULL
/// lookup.
fn load(
    fs: &FakeFs,
    module: bool,
    data: Option<&str>,
    def: Option<&str>,
    locale: Option<&str>,
) -> Option<(&'static str, Option<&'static str>)> {
    if !module {
        return None;
    }
    let (def, locale) = (def?.as_bytes(), locale?.as_bytes());
    let data = data.map(str::as_bytes);
    let first = find_file(fs, data, Some(&default_file(Some(def))))?;
    let (mut a, mut b) = fs.text(&first);
    if is_default(Some(locale), Some(def)) {
        return Some((a, b));
    }
    if let Some(second) = find_file(fs, data, Some(&locale_file(Some(locale)))) {
        let (a2, b2) = fs.text(&second);
        a = a2;
        b = b2.or(b);
    }
    Some((a, b))
}

/// `obs_find_module_file()` of the module with the data path `data`.
fn found(fs: &FakeFs, data: &str, file: &str) -> Option<String> {
    find_file(fs, Some(data.as_bytes()), Some(file.as_bytes()))
        .map(|p| String::from_utf8_lossy(&p).into_owned())
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn module_locale() {
    let fs = FakeFs::module_locale();
    let loc = Some("/R/loc");
    let l = |def, locale| load(&fs, true, loc, def, locale);

    // The default only.
    assert_eq!(l(Some("en-US"), Some("en-US")), Some(EN));
    // Another locale on top of the default.
    assert_eq!(
        l(Some("en-US"), Some("de-DE")),
        Some(("de A", Some("en B")))
    );
    // The default in another case: not loaded, though its file exists
    // (characterized, not endorsed).
    assert_eq!(l(Some("en-US"), Some("EN-us")), Some(EN));
    assert_eq!(l(Some("EN-us"), Some("en-US")), Some(EN_CASE));
    // A missing locale keeps the default.
    assert_eq!(l(Some("en-US"), Some("fr-FR")), Some(EN));
    assert_eq!(l(Some("en-US"), Some("")), Some(EN));
    // A missing default: NULL, though the locale exists.
    assert_eq!(l(Some("xx-XX"), Some("de-DE")), None);
    assert_eq!(l(Some(""), Some("de-DE")), None);
    assert_eq!(
        load(&fs, true, Some("/R/nodef"), Some("en-US"), Some("de-DE")),
        None
    );
    assert_eq!(
        load(&fs, true, Some("/R/none"), Some("en-US"), Some("en-US")),
        None
    );
    // The default file is a second locale's file as well.
    assert_eq!(l(Some("de-DE"), Some("en-US")), Some(EN));
    assert_eq!(l(Some("de-DE"), Some("de-DE")), Some(DE));

    // NULL parameters.
    assert_eq!(load(&fs, false, loc, Some("en-US"), Some("en-US")), None);
    assert_eq!(l(None, Some("en-US")), None);
    assert_eq!(l(Some("en-US"), None), None);
    assert_eq!(l(None, None), None);

    // The two names as obs_find_module_file() resolves them
    // (characterized, not endorsed: the second has two '/').
    assert_eq!(
        found(&fs, "/R/loc", "locale/de-DE.ini").as_deref(),
        Some("/R/loc/locale/de-DE.ini")
    );
    assert_eq!(
        found(&fs, "/R/loc", "/locale/de-DE.ini").as_deref(),
        Some("/R/loc//locale/de-DE.ini")
    );

    // A data path with a trailing '/': the same text.
    assert_eq!(
        load(&fs, true, Some("/R/loc/"), Some("en-US"), Some("de-DE")),
        Some(("de A", Some("en B")))
    );
    assert_eq!(
        found(&fs, "/R/loc/", "/locale/de-DE.ini").as_deref(),
        Some("/R/loc//locale/de-DE.ini")
    );

    // An empty data path: the default from the working directory, the
    // locale from the root, so the working directory's de-DE is not read
    // (characterized, not endorsed).
    assert_eq!(
        load(&fs, true, Some(""), Some("en-US"), Some("de-DE")).map(|t| t.0),
        Some("en A")
    );
}

#[test]
fn load_asks_for_the_two_names_in_order() {
    let fs = FakeFs::module_locale();
    load(&fs, true, Some("/R/loc"), Some("en-US"), Some("de-DE"));
    assert_eq!(
        *fs.asked.borrow(),
        ["/R/loc/locale/en-US.ini", "/R/loc//locale/de-DE.ini"]
    );
    // An empty data path: relative, then absolute.
    let fs = FakeFs::module_locale();
    load(&fs, true, Some(""), Some("en-US"), Some("de-DE"));
    assert_eq!(
        *fs.asked.borrow(),
        ["locale/en-US.ini", "/locale/de-DE.ini"]
    );
    // The default: one name; a missing default: one name.
    for def in ["en-US", "xx-XX"] {
        let fs = FakeFs::module_locale();
        load(&fs, true, Some("/R/loc"), Some(def), Some("en-us"));
        assert_eq!(fs.asked.borrow().len(), 1, "{def}");
    }
}

#[test]
fn default_file_edges() {
    assert_eq!(s(&default_file(Some(b"en-US"))), "locale/en-US.ini");
    assert_eq!(s(&default_file(Some(b""))), "locale/.ini");
    assert_eq!(s(&default_file(None)), "locale/.ini");
    assert_eq!(s(&default_file(Some(b"/abs"))), "locale//abs.ini");
    assert_eq!(s(&default_file(Some(b"../x"))), "locale/../x.ini");
    assert_eq!(s(&default_file(Some(b"x.ini"))), "locale/x.ini.ini");
    assert_eq!(default_file(Some(b"\xff\x80")), b"locale/\xff\x80.ini");
}

#[test]
fn locale_file_edges() {
    assert_eq!(s(&locale_file(Some(b"de-DE"))), "/locale/de-DE.ini");
    assert_eq!(s(&locale_file(Some(b""))), "/locale/.ini");
    assert_eq!(s(&locale_file(None)), "/locale/.ini");
    assert_eq!(s(&locale_file(Some(b"/abs"))), "/locale//abs.ini");
    assert_eq!(s(&locale_file(Some(b"../x"))), "/locale/../x.ini");
    assert_eq!(locale_file(Some(b"\xff\x80")), b"/locale/\xff\x80.ini");
}

#[test]
fn is_default_edges() {
    assert!(is_default(Some(b"en-US"), Some(b"en-US")));
    assert!(is_default(Some(b"EN-us"), Some(b"en-US")));
    assert!(is_default(Some(b"en-us"), Some(b"EN-US")));
    assert!(!is_default(Some(b"de-DE"), Some(b"en-US")));
    // A prefix is not the default.
    assert!(!is_default(Some(b"en"), Some(b"en-US")));
    assert!(!is_default(Some(b"en-US"), Some(b"en")));
    assert!(!is_default(Some(b"en-US "), Some(b"en-US")));
    // NULL is empty.
    assert!(is_default(None, None));
    assert!(is_default(None, Some(b"")));
    assert!(is_default(Some(b""), None));
    assert!(!is_default(None, Some(b"en-US")));
    assert!(!is_default(Some(b"en-US"), None));
    // Only ASCII is folded: "é" and "É" (UTF-8) differ.
    assert!(!is_default(Some("é".as_bytes()), Some("É".as_bytes())));
    assert!(is_default(Some(b"\xff"), Some(b"\xff")));
    // '_' and '-' differ, as do '@' and '`' (one off 'A' and 'a').
    assert!(!is_default(Some(b"en_US"), Some(b"en-US")));
    assert!(!is_default(Some(b"@"), Some(b"`")));
}

// ------------------------------------------------------------ proptests

/// Locale bytes, NUL excluded (C strings).
fn locale() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            Just(b'/'),
            Just(b'.'),
            Just(b'-'),
            Just(b'_'),
            prop::sample::select(b"enUSdeDE".to_vec()),
            1u8..=255,
        ],
        0..10,
    )
}

fn opt(v: Vec<u8>, null: bool) -> Option<Vec<u8>> {
    (!null).then_some(v)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn default_file_is_the_prefix_locale_and_ext(l in locale(), null in any::<bool>()) {
        let l = opt(l, null);
        let want = [DEFAULT_PREFIX, l.as_deref().unwrap_or_default(), LOCALE_EXT].concat();
        prop_assert_eq!(default_file(l.as_deref()), want);
    }

    #[test]
    fn locale_file_is_the_default_file_after_a_separator(l in locale(), null in any::<bool>()) {
        let l = opt(l, null);
        let mut want = vec![b'/'];
        want.extend_from_slice(&default_file(l.as_deref()));
        prop_assert_eq!(locale_file(l.as_deref()), want.clone());
        prop_assert!(locale_file(l.as_deref()).starts_with(LOCALE_PREFIX));
    }

    #[test]
    fn is_default_ignores_ascii_case(a in locale(), b in locale(), nulls in any::<[bool; 2]>()) {
        let (a, b) = (opt(a, nulls[0]), opt(b, nulls[1]));
        let (x, y) = (a.as_deref().unwrap_or_default(), b.as_deref().unwrap_or_default());
        prop_assert_eq!(is_default(a.as_deref(), b.as_deref()), x.eq_ignore_ascii_case(y));
        prop_assert_eq!(is_default(a.as_deref(), b.as_deref()), is_default(b.as_deref(), a.as_deref()));
    }

    #[test]
    fn a_locale_in_another_case_is_the_default(a in locale()) {
        prop_assert!(is_default(Some(&a.to_ascii_uppercase()), Some(&a.to_ascii_lowercase())));
    }
}
