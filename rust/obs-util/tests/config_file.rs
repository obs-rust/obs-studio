//! Tier 1: safe-core tests for `libobs/util/config-file.c`.
//!
//! Each test names the cmocka case in `test/cmocka/test_config_file.c` (or
//! `test/cmocka/test_config_save.c`) it mirrors. Assertions that need a
//! NULL `config_t *` go through the FFI shim, the only place NULL exists.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use obs_c_oracle as _;
use obs_util::config_file::{CONFIG_ERROR, CONFIG_FILENOTFOUND, CONFIG_SUCCESS, Config, OpenType};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "obs-config-file-{}-{}-{n}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_str().unwrap().as_bytes().to_vec()
    }
}

fn open_str(str_: &str) -> Config {
    Config::open_string(str_.as_bytes())
}

fn expect_str(config: &Config, section: &str, name: &str, expected: &[u8]) {
    let value = config.get_string(section.as_bytes(), name.as_bytes());
    assert_eq!(value.as_deref(), Some(expected), "{section} / {name}");
}

fn expect_missing(config: &Config, section: &str, name: &str) {
    assert!(
        config
            .get_string(section.as_bytes(), name.as_bytes())
            .is_none(),
        "{section} / {name}"
    );
}

fn expect_section(config: &Config, idx: usize, expected: &str) {
    assert_eq!(
        config.get_section(idx).as_deref(),
        Some(expected.as_bytes())
    );
}

fn write_file(path: &Path, text: &[u8]) {
    std::fs::write(path, text).unwrap();
}

/// Reads like C's `expect_file`: `os_fread_utf8` strips a leading UTF-8
/// BOM, which `config_save` emits on Windows.
fn read_file_utf8(path: &Path) -> Vec<u8> {
    let mut data = std::fs::read(path).unwrap();
    if data.starts_with(b"\xEF\xBB\xBF") {
        data.drain(..3);
    }
    data
}

fn expect_file(path: &Path, expected: &[u8]) {
    assert_eq!(read_file_utf8(path), expected);
}

fn expect_file_empty(path: &Path) {
    assert!(path.exists());
    assert_eq!(read_file_utf8(path), b"");
}

/* ---------------------------------------------------------------------- */
/* parsing */

/// `test_parse_basic`.
#[test]
fn test_parse_basic() {
    let c = open_str("[General]\nname=OBS\ncount=42\n\n[Video]\nfps=60\n[Audio]\nrate=48000\n");

    assert_eq!(c.num_sections(), 3);
    expect_section(&c, 0, "General");
    expect_section(&c, 1, "Video");
    expect_section(&c, 2, "Audio");
    assert!(c.get_section(3).is_none());
    assert!(c.get_section(100).is_none());

    expect_str(&c, "General", "name", b"OBS");
    expect_str(&c, "General", "count", b"42");
    expect_str(&c, "Video", "fps", b"60");
    expect_str(&c, "Audio", "rate", b"48000");

    expect_missing(&c, "general", "name");
    expect_missing(&c, "General", "Name");
    expect_missing(&c, "General", "fps");
    expect_missing(&c, "Nope", "name");
}

/// `test_parse_empty_string`.
#[test]
fn test_parse_empty_string() {
    let c = open_str("");
    assert_eq!(c.num_sections(), 0);
    assert!(c.get_section(0).is_none());

    let c = open_str("\n\n   \n\t\n");
    assert_eq!(c.num_sections(), 0);
}

/// `test_parse_whitespace_is_kept` (characterized, not endorsed).
#[test]
fn test_parse_whitespace_is_kept() {
    let c = open_str("[ Sp ace ]\n  key one = value two \n\tk2\t=\tv2\n  lead=x\n");

    assert_eq!(c.num_sections(), 1);
    expect_section(&c, 0, " Sp ace ");
    expect_str(&c, " Sp ace ", "key one ", b" value two ");
    expect_str(&c, " Sp ace ", "k2\t", b"\tv2");
    expect_str(&c, " Sp ace ", "lead", b"x");

    expect_missing(&c, " Sp ace ", "key one");
    expect_missing(&c, "Sp ace", "key one ");
}

/// `test_parse_crlf_line_endings`.
#[test]
fn test_parse_crlf_line_endings() {
    let c = open_str("[A]\r\nk=v\r\n\r\n[B]\r\nj=w\r\n");

    assert_eq!(c.num_sections(), 2);
    expect_section(&c, 0, "A");
    expect_section(&c, 1, "B");
    expect_str(&c, "A", "k", b"v");
    expect_str(&c, "B", "j", b"w");
}

/// `test_parse_comments` ("; semi" case characterized, not endorsed).
#[test]
fn test_parse_comments() {
    let c = open_str(
        "# top comment\n; top semicolon\n[S]\n# hash comment\n#x=2\n; semi=1\nreal=3\nv=a # not\n",
    );

    assert_eq!(c.num_sections(), 1);
    expect_section(&c, 0, "S");
    expect_missing(&c, "S", "x");
    expect_missing(&c, "S", "#x");
    expect_str(&c, "S", "real", b"3");
    expect_str(&c, "S", "; semi", b"1");
    expect_missing(&c, "S", "semi");
    expect_str(&c, "S", "v", b"a # not");
}

/// `test_parse_empty_values`.
#[test]
fn test_parse_empty_values() {
    let c = open_str("[S]\nempty=\nafter=x\neof=");

    expect_str(&c, "S", "empty", b"");
    expect_str(&c, "S", "after", b"x");
    expect_str(&c, "S", "eof", b"");
}

/// `test_parse_no_trailing_newline`.
#[test]
fn test_parse_no_trailing_newline() {
    let c = open_str("[S]\nk=v");
    expect_str(&c, "S", "k", b"v");
}

/// `test_parse_keys_before_section`.
#[test]
fn test_parse_keys_before_section() {
    let c = open_str("orphan=1\n; semi\nmore stuff\n[S]\nk=v\n");

    assert_eq!(c.num_sections(), 1);
    expect_section(&c, 0, "S");
    expect_str(&c, "S", "k", b"v");
    expect_missing(&c, "S", "orphan");
    expect_missing(&c, "", "orphan");

    let c = open_str("a=1\nb=2\n");
    assert_eq!(c.num_sections(), 0);
}

/// `test_parse_lines_without_equals`.
#[test]
fn test_parse_lines_without_equals() {
    let c = open_str("[S]\nnoequals\nk=v\njust some words\nk2=v2\n=novalue\n");

    assert_eq!(c.num_sections(), 1);
    expect_str(&c, "S", "k", b"v");
    expect_str(&c, "S", "k2", b"v2");
    expect_missing(&c, "S", "noequals");
    expect_missing(&c, "S", "just some words");
    expect_missing(&c, "S", "");
    expect_missing(&c, "S", "=novalue");
}

/// `test_parse_last_line_without_equals_at_eof` (characterized, not endorsed).
#[test]
fn test_parse_last_line_without_equals_at_eof() {
    let c = open_str("[S]\nk=v\nlast words");

    expect_str(&c, "S", "k", b"v");
    expect_str(&c, "S", "last words", b"");
}

/// `test_parse_unterminated_header` (characterized, not endorsed).
#[test]
fn test_parse_unterminated_header() {
    let c = open_str("[bad\nk=v\n");

    assert_eq!(c.num_sections(), 1);
    expect_section(&c, 0, "bad");
    expect_str(&c, "bad", "k", b"v");

    let c = open_str("[S]\nk=v\n[unterminated");
    assert_eq!(c.num_sections(), 2);
    expect_section(&c, 0, "S");
    expect_section(&c, 1, "unterminated");
}

/// `test_parse_empty_header_stops_parsing` (characterized, not endorsed).
#[test]
fn test_parse_empty_header_stops_parsing() {
    let c = open_str("[S]\nk=v\n[]\n[T]\nj=w\n");

    assert_eq!(c.num_sections(), 1);
    expect_section(&c, 0, "S");
    expect_str(&c, "S", "k", b"v");
    expect_missing(&c, "T", "j");

    let c = open_str("[]\n[X]\nk=v\n");
    assert_eq!(c.num_sections(), 0);
}

/// `test_parse_trailing_text_after_header`.
#[test]
fn test_parse_trailing_text_after_header() {
    let c = open_str("[S] junk\nk=v\n");

    assert_eq!(c.num_sections(), 1);
    expect_str(&c, "S", "k", b"v");
    expect_missing(&c, "S", "junk");
}

/// `test_parse_duplicates` (characterized, not endorsed).
#[test]
fn test_parse_duplicates() {
    let c = open_str("[S]\nk=1\nk=2\n[S]\nb=3\n[T]\nk=4\n");

    assert_eq!(c.num_sections(), 3);
    expect_section(&c, 0, "S");
    expect_section(&c, 1, "S");
    expect_section(&c, 2, "T");

    expect_str(&c, "S", "b", b"3");
    expect_missing(&c, "S", "k");
    expect_str(&c, "T", "k", b"4");

    let c = open_str("[S]\nk=1\nk=2\n");
    assert_eq!(c.num_sections(), 1);
    expect_str(&c, "S", "k", b"2");
}

/// `test_parse_escapes`.
#[test]
fn test_parse_escapes() {
    let c = open_str("[S]\nnl=a\\nb\ncr=a\\rb\nbs=a\\\\b\nunk=a\\tb\ntrail=a\\\n");

    expect_str(&c, "S", "nl", b"a\nb");
    expect_str(&c, "S", "cr", b"a\rb");
    expect_str(&c, "S", "bs", b"a\\b");
    expect_str(&c, "S", "unk", b"a\\tb");
    expect_str(&c, "S", "trail", b"a\\");
}

/* ---------------------------------------------------------------------- */
/* typed getters */

/// `test_get_int_parsing` ("neguint" wrap characterized, not endorsed).
#[test]
fn test_get_int_parsing() {
    let c = open_str(
        "[N]\nhex=0x1F\nbigx=0X1F\njunk=12abc\nalpha=abc\nneg=-5\nempty=\nsp=  7\nneguint=-1\ndec=3.9\nzero=0\n",
    );

    assert_eq!(c.get_int(b"N", b"hex"), 31);
    assert_eq!(c.get_int(b"N", b"bigx"), 0);
    assert_eq!(c.get_int(b"N", b"junk"), 12);
    assert_eq!(c.get_int(b"N", b"alpha"), 0);
    assert_eq!(c.get_int(b"N", b"neg"), -5);
    assert_eq!(c.get_int(b"N", b"empty"), 0);
    assert_eq!(c.get_int(b"N", b"sp"), 7);
    assert_eq!(c.get_int(b"N", b"dec"), 3);
    assert_eq!(c.get_int(b"N", b"zero"), 0);
    assert_eq!(c.get_int(b"N", b"missing"), 0);
    assert_eq!(c.get_int(b"Missing", b"hex"), 0);

    assert_eq!(c.get_uint(b"N", b"hex"), 31);
    assert_eq!(c.get_uint(b"N", b"junk"), 12);
    assert_eq!(c.get_uint(b"N", b"alpha"), 0);
    assert_eq!(c.get_uint(b"N", b"missing"), 0);
    assert_eq!(c.get_uint(b"N", b"neguint"), u64::MAX);
}

/// `test_get_bool_parsing`.
#[test]
fn test_get_bool_parsing() {
    let c = open_str(
        "[B]\nt=true\nT=TRUE\none=1\ntwo=2\nzero=0\nf=false\nyes=yes\nhex=0x1\nneg=-1\nempty=\nsp= true\n",
    );

    assert!(c.get_bool(b"B", b"t"));
    assert!(c.get_bool(b"B", b"T"));
    assert!(c.get_bool(b"B", b"one"));
    assert!(c.get_bool(b"B", b"two"));
    assert!(c.get_bool(b"B", b"hex"));
    assert!(c.get_bool(b"B", b"neg"));

    assert!(!c.get_bool(b"B", b"zero"));
    assert!(!c.get_bool(b"B", b"f"));
    assert!(!c.get_bool(b"B", b"yes"));
    assert!(!c.get_bool(b"B", b"empty"));
    assert!(!c.get_bool(b"B", b"sp"));
    assert!(!c.get_bool(b"B", b"missing"));
}

/// `test_get_double_parsing`.
#[test]
fn test_get_double_parsing() {
    let c = open_str("[D]\na=1.5\nb=abc\nc=1e3\nd=-0.5\ne= 2.5\nf=\n");

    assert_eq!(c.get_double(b"D", b"a"), 1.5);
    assert_eq!(c.get_double(b"D", b"b"), 0.0);
    assert_eq!(c.get_double(b"D", b"c"), 1000.0);
    assert_eq!(c.get_double(b"D", b"d"), -0.5);
    assert_eq!(c.get_double(b"D", b"e"), 2.5);
    assert_eq!(c.get_double(b"D", b"f"), 0.0);
    assert_eq!(c.get_double(b"D", b"missing"), 0.0);
}

/* ---------------------------------------------------------------------- */
/* setters */

/// `test_set_get_types`.
#[test]
fn test_set_get_types() {
    let c = open_str("");

    c.set_string(b"S", b"str", b"hello");
    expect_str(&c, "S", "str", b"hello");

    /* a NULL string is stored as ""; the safe API has no NULL, but an
    empty slice is the same store */
    c.set_string(b"S", b"null", b"");
    expect_str(&c, "S", "null", b"");

    c.set_int(b"S", b"int", -42);
    expect_str(&c, "S", "int", b"-42");
    assert_eq!(c.get_int(b"S", b"int"), -42);

    c.set_uint(b"S", b"uint", u64::MAX);
    expect_str(&c, "S", "uint", b"18446744073709551615");
    assert_eq!(c.get_uint(b"S", b"uint"), u64::MAX);

    c.set_bool(b"S", b"yes", true);
    c.set_bool(b"S", b"no", false);
    expect_str(&c, "S", "yes", b"true");
    expect_str(&c, "S", "no", b"false");
    assert!(c.get_bool(b"S", b"yes"));
    assert!(!c.get_bool(b"S", b"no"));

    c.set_double(b"S", b"d1", 1.5);
    c.set_double(b"S", b"d2", 2.0);
    c.set_double(b"S", b"d3", 0.1);
    expect_str(&c, "S", "d1", b"1.5");
    expect_str(&c, "S", "d2", b"2.0");
    expect_str(&c, "S", "d3", b"0.10000000000000001");
    assert_eq!(c.get_double(b"S", b"d1"), 1.5);
    assert_eq!(c.get_double(b"S", b"d2"), 2.0);
    assert_eq!(c.get_double(b"S", b"d3"), 0.1);

    c.set_string(b"S", b"str", b"again");
    expect_str(&c, "S", "str", b"again");
    assert_eq!(c.num_sections(), 1);
}

/// `test_set_adds_sections_in_order`.
#[test]
fn test_set_adds_sections_in_order() {
    let c = open_str("[B]\nk=1\n[A]\nk=2\n");

    c.set_string(b"Z", b"k", b"3");
    c.set_string(b"B", b"j", b"4");

    assert_eq!(c.num_sections(), 3);
    expect_section(&c, 0, "B");
    expect_section(&c, 1, "A");
    expect_section(&c, 2, "Z");
    expect_str(&c, "B", "j", b"4");
}

/// `test_remove_value`.
#[test]
fn test_remove_value() {
    let c = open_str("[S]\na=1\nb=2\n");

    assert!(c.remove_value(b"S", b"a"));
    expect_missing(&c, "S", "a");
    expect_str(&c, "S", "b", b"2");
    assert!(!c.has_user_value(b"S", b"a"));

    assert!(!c.remove_value(b"S", b"a"));
    assert!(!c.remove_value(b"S", b"nope"));
    assert!(!c.remove_value(b"Nope", b"b"));

    assert!(c.remove_value(b"S", b"b"));
    assert_eq!(c.num_sections(), 1);
    expect_section(&c, 0, "S");
}

/* ---------------------------------------------------------------------- */
/* defaults */

/// `test_defaults_become_user_values` (characterized, not endorsed).
#[test]
fn test_defaults_become_user_values() {
    let c = open_str("");

    assert!(!c.has_default_value(b"D", b"i"));
    assert!(!c.has_user_value(b"D", b"i"));

    c.set_default_int(b"D", b"i", 7);
    assert!(c.has_default_value(b"D", b"i"));
    assert!(c.has_user_value(b"D", b"i"));
    assert_eq!(c.num_sections(), 1);
    assert_eq!(c.get_int(b"D", b"i"), 7);
    assert_eq!(c.get_default_int(b"D", b"i"), 7);

    c.set_int(b"D", b"i", 9);
    assert_eq!(c.get_int(b"D", b"i"), 9);
    assert_eq!(c.get_default_int(b"D", b"i"), 7);

    c.set_default_int(b"D", b"i", 8);
    assert_eq!(c.get_int(b"D", b"i"), 9);
    assert_eq!(c.get_default_int(b"D", b"i"), 8);
}

/// `test_defaults_user_value_wins`.
#[test]
fn test_defaults_user_value_wins() {
    let c = open_str("");

    c.set_string(b"D", b"s", b"user");
    c.set_default_string(b"D", b"s", b"dflt");

    expect_str(&c, "D", "s", b"user");
    assert_eq!(
        c.get_default_string(b"D", b"s").as_deref(),
        Some(b"dflt".as_slice())
    );
    assert!(c.has_user_value(b"D", b"s"));
    assert!(c.has_default_value(b"D", b"s"));

    assert!(c.remove_value(b"D", b"s"));
    assert!(!c.has_user_value(b"D", b"s"));
    assert!(c.has_default_value(b"D", b"s"));
    expect_str(&c, "D", "s", b"dflt");
    assert!(!c.remove_value(b"D", b"s"));
}

/// `test_defaults_typed`.
#[test]
fn test_defaults_typed() {
    let c = open_str("");

    c.set_default_uint(b"T", b"u", 5);
    c.set_default_bool(b"T", b"yes", true);
    c.set_default_bool(b"T", b"no", false);
    c.set_default_double(b"T", b"whole", 3.0);
    c.set_default_double(b"T", b"frac", 0.25);
    c.set_default_string(b"T", b"null", b"");

    assert_eq!(c.get_default_uint(b"T", b"u"), 5);
    assert!(c.get_default_bool(b"T", b"yes"));
    assert!(!c.get_default_bool(b"T", b"no"));
    assert_eq!(
        c.get_default_string(b"T", b"yes").as_deref(),
        Some(b"true".as_slice())
    );
    assert_eq!(
        c.get_default_string(b"T", b"no").as_deref(),
        Some(b"false".as_slice())
    );
    assert_eq!(
        c.get_default_string(b"T", b"whole").as_deref(),
        Some(b"3".as_slice())
    );
    assert_eq!(
        c.get_default_string(b"T", b"frac").as_deref(),
        Some(b"0.25".as_slice())
    );
    assert_eq!(c.get_default_double(b"T", b"frac"), 0.25);
    assert_eq!(c.get_default_int(b"T", b"whole"), 3);
    assert_eq!(
        c.get_default_string(b"T", b"null").as_deref(),
        Some(b"".as_slice())
    );

    assert!(c.get_default_string(b"T", b"missing").is_none());
    assert!(c.get_default_string(b"Missing", b"u").is_none());
    assert_eq!(c.get_default_int(b"T", b"missing"), 0);
    assert_eq!(c.get_default_uint(b"T", b"missing"), 0);
    assert!(!c.get_default_bool(b"T", b"missing"));
    assert_eq!(c.get_default_double(b"T", b"missing"), 0.0);
    assert!(!c.has_default_value(b"T", b"missing"));
    assert!(!c.has_user_value(b"Missing", b"u"));
}

/// `test_open_defaults_file`.
#[test]
fn test_open_defaults_file() {
    let scratch = Scratch::new();
    let defaults = scratch.join("defaults.ini");
    write_file(&defaults, b"[Def]\na=1\nb=true\n[Other]\nc=x\n");

    let c = open_str("");

    assert_eq!(c.open_defaults(&path_bytes(&defaults)), CONFIG_SUCCESS);

    assert_eq!(c.num_sections(), 0);
    assert!(c.has_default_value(b"Def", b"a"));
    assert!(!c.has_user_value(b"Def", b"a"));
    expect_str(&c, "Def", "a", b"1");
    assert_eq!(c.get_default_int(b"Def", b"a"), 1);
    assert!(c.get_default_bool(b"Def", b"b"));
    expect_str(&c, "Other", "c", b"x");

    c.set_string(b"Def", b"a", b"9");
    assert_eq!(c.get_int(b"Def", b"a"), 9);
    assert_eq!(c.get_default_int(b"Def", b"a"), 1);
    assert_eq!(c.num_sections(), 1);
}

/// `test_open_defaults_errors` (NULL-config part is a C ABI case).
#[test]
fn test_open_defaults_errors() {
    let scratch = Scratch::new();
    let missing = scratch.join("missing.ini");

    let c = open_str("");

    assert_eq!(c.open_defaults(&path_bytes(&missing)), CONFIG_FILENOTFOUND);
    assert!(!missing.exists());
}

/* ---------------------------------------------------------------------- */
/* files */

/// `test_open_null_config`: a null `config_t **` fails with CONFIG_ERROR
/// before touching the file, and `config_close(NULL)` is a no-op.
#[test]
fn test_open_null_config() {
    use obs_util::ffi::config_file::{config_close, config_data, config_open, config_open_string};
    use std::ffi::CString;
    let scratch = Scratch::new();
    let path = scratch.join("open.ini");
    let cpath = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: null pointers; the shim must reject them without touching
    // the filesystem.
    unsafe {
        assert_eq!(
            config_open(core::ptr::null_mut::<*mut config_data>(), cpath.as_ptr(), 1),
            CONFIG_ERROR
        );
        assert_eq!(
            config_open_string(core::ptr::null_mut::<*mut config_data>(), c"[S]\n".as_ptr()),
            CONFIG_ERROR
        );
        config_close(core::ptr::null_mut());
    }
    assert!(!path.exists());
}

/// `test_open_existing_missing`: C nulls the output pointer on failure;
/// the safe API returns `Err` instead.
#[test]
fn test_open_existing_missing() {
    let scratch = Scratch::new();
    let missing = scratch.join("missing.ini");

    assert_eq!(
        Config::open(&path_bytes(&missing), OpenType::Existing).unwrap_err(),
        CONFIG_FILENOTFOUND
    );
    assert!(!missing.exists());
}

/// `test_open_existing_file`.
#[test]
fn test_open_existing_file() {
    let scratch = Scratch::new();
    let path = scratch.join("open.ini");
    write_file(&path, b"[S]\nk=v\n");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    assert_eq!(c.num_sections(), 1);
    expect_str(&c, "S", "k", b"v");
    drop(c);

    let c = Config::open(&path_bytes(&path), OpenType::Always).unwrap();
    expect_str(&c, "S", "k", b"v");
    drop(c);
    expect_file(&path, b"[S]\nk=v\n");
}

/// `test_open_always_creates_file`.
#[test]
fn test_open_always_creates_file() {
    let scratch = Scratch::new();
    let path = scratch.join("always.ini");
    assert!(!path.exists());

    let c = Config::open(&path_bytes(&path), OpenType::Always).unwrap();
    assert_eq!(c.num_sections(), 0);
    assert!(path.exists());
    expect_file_empty(&path);
}

/// `test_open_empty_file`.
#[test]
fn test_open_empty_file() {
    let scratch = Scratch::new();
    let path = scratch.join("empty.ini");
    write_file(&path, b"");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    assert_eq!(c.num_sections(), 0);
}

/// `test_create`.
#[test]
fn test_create() {
    let scratch = Scratch::new();
    let path = scratch.join("create.ini");
    write_file(&path, b"[Old]\nk=v\n");

    let c = Config::create(&path_bytes(&path)).unwrap();
    assert_eq!(c.num_sections(), 0);
    expect_file_empty(&path);
    drop(c);

    let bad = scratch.join("nodir/x.ini");
    assert!(Config::create(&path_bytes(&bad)).is_none());
    assert!(!bad.exists());
}

/// `test_save_exact_text`.
#[test]
fn test_save_exact_text() {
    let scratch = Scratch::new();
    let path = scratch.join("create.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();

    c.set_string(b"Video", b"fps", b"60");
    c.set_int(b"Audio", b"rate", 48000);
    c.set_string(b"Video", b"res", b"1920x1080");
    c.set_bool(b"Audio", b"mono", false);
    c.set_string(b"Video", b"path", b"a\\b\nc\rd");
    c.set_double(b"Misc", b"ratio", 1.5);

    assert_eq!(c.save(), CONFIG_SUCCESS);
    expect_file(
        &path,
        b"[Video]\nfps=60\nres=1920x1080\npath=a\\\\b\\nc\\rd\n\n[Audio]\nrate=48000\nmono=false\n\n[Misc]\nratio=1.5\n",
    );
    drop(c);

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    assert_eq!(c.num_sections(), 3);
    expect_section(&c, 0, "Video");
    expect_section(&c, 1, "Audio");
    expect_section(&c, 2, "Misc");
    expect_str(&c, "Video", "path", b"a\\b\nc\rd");
    assert_eq!(c.get_int(b"Audio", b"rate"), 48000);
    assert!(!c.get_bool(b"Audio", b"mono"));
    assert_eq!(c.get_double(b"Misc", b"ratio"), 1.5);
}

/// `test_save_overwrites_file`.
#[test]
fn test_save_overwrites_file() {
    let scratch = Scratch::new();
    let path = scratch.join("open.ini");
    write_file(&path, b"[S]\nk=old\nextra=gone\n");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    c.set_string(b"S", b"k", b"new");
    assert!(c.remove_value(b"S", b"extra"));
    assert_eq!(c.save(), CONFIG_SUCCESS);
    drop(c);

    expect_file(&path, b"[S]\nk=new\n");
}

/// `test_save_empty_config`.
#[test]
fn test_save_empty_config() {
    let scratch = Scratch::new();
    let path = scratch.join("create.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();
    assert_eq!(c.save(), CONFIG_SUCCESS);
    expect_file_empty(&path);
}

/// `test_save_empty_section`.
#[test]
fn test_save_empty_section() {
    let scratch = Scratch::new();
    let path = scratch.join("create.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();
    c.set_string(b"S", b"k", b"v");
    assert!(c.remove_value(b"S", b"k"));
    assert_eq!(c.save(), CONFIG_SUCCESS);
    expect_file(&path, b"[S]\n");
}

/// `test_save_invalid_config`: `config_save(NULL)` is a C ABI case; the
/// string-opened config part mirrors here.
#[test]
fn test_save_invalid_config() {
    let c = open_str("[S]\nk=v\n");
    assert_eq!(c.save(), CONFIG_ERROR);
}

/// `test_save_writes_defaults_as_user_values` (characterized, not endorsed).
#[test]
fn test_save_writes_defaults_as_user_values() {
    let scratch = Scratch::new();
    let path = scratch.join("defsave.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();
    c.set_default_int(b"A", b"x", 1);
    c.set_string(b"A", b"y", b"2");
    assert_eq!(c.save(), CONFIG_SUCCESS);

    expect_file(&path, b"[A]\nx=1\ny=2\n");
}

/// `test_roundtrip_malformed`.
#[test]
fn test_roundtrip_malformed() {
    let scratch = Scratch::new();
    let path = scratch.join("roundtrip.ini");
    write_file(
        &path,
        b"orphan=1\n# top comment\n[First]\n; semi=2\n#hash=3\nnoequals\nkey = spaced \nk=a\nk=b\n\n[Empty]\n[First]\nlate=4\nlast",
    );

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    assert_eq!(c.num_sections(), 3);
    assert_eq!(c.save(), CONFIG_SUCCESS);
    drop(c);

    expect_file(
        &path,
        b"[First]\n; semi=2\nkey = spaced \nk=a\nk=b\n\n[Empty]\n\n[First]\nlate=4\nlast=\n",
    );
}

/// `test_roundtrip_crlf_input`.
#[test]
fn test_roundtrip_crlf_input() {
    let scratch = Scratch::new();
    let path = scratch.join("roundtrip.ini");
    write_file(&path, b"[A]\r\nk=v\r\n\r\n[B]\r\nj=w\r\n");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    assert_eq!(c.save(), CONFIG_SUCCESS);
    drop(c);

    expect_file(&path, b"[A]\nk=v\n\n[B]\nj=w\n");
}

/// `test_roundtrip_unknown_escape` (characterized, not endorsed).
#[test]
fn test_roundtrip_unknown_escape() {
    let scratch = Scratch::new();
    let path = scratch.join("escape.ini");
    write_file(&path, b"[S]\nk=a\\tb\nn=a\\nb\n");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    expect_str(&c, "S", "k", b"a\\tb");
    assert_eq!(c.save(), CONFIG_SUCCESS);
    drop(c);

    expect_file(&path, b"[S]\nk=a\\\\tb\nn=a\\nb\n");
}

/* ---------------------------------------------------------------------- */
/* config_save_safe */

/// `test_save_safe_with_backup`.
#[test]
fn test_save_safe_with_backup() {
    let scratch = Scratch::new();
    let path = scratch.join("safe1.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();
    c.set_string(b"Old", b"k", b"1");
    assert_eq!(c.save(), CONFIG_SUCCESS);

    c.set_string(b"Old", b"k", b"2");
    assert_eq!(c.save_safe(Some(b"tmp"), Some(b"bak")), CONFIG_SUCCESS);
    drop(c);

    expect_file(&path, b"[Old]\nk=2\n");
    expect_file(&path.with_extension("ini.bak"), b"[Old]\nk=1\n");
    assert!(!path.with_extension("ini.tmp").exists());
}

/// `test_save_safe_dotted_extensions`.
#[test]
fn test_save_safe_dotted_extensions() {
    let scratch = Scratch::new();
    let path = scratch.join("safe2.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();
    c.set_string(b"Old", b"k", b"1");
    assert_eq!(c.save(), CONFIG_SUCCESS);

    c.set_string(b"Old", b"k", b"2");
    assert_eq!(c.save_safe(Some(b".tmp"), Some(b".bak")), CONFIG_SUCCESS);
    drop(c);

    expect_file(&path, b"[Old]\nk=2\n");
    expect_file(&scratch.join("safe2.ini.bak"), b"[Old]\nk=1\n");
    assert!(!scratch.join("safe2.ini.tmp").exists());
}

/// `test_save_safe_without_backup`.
#[test]
fn test_save_safe_without_backup() {
    let scratch = Scratch::new();
    let path = scratch.join("safe3.ini");
    write_file(&path, b"[A]\nx=1\n");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    c.set_int(b"A", b"x", 2);
    assert_eq!(c.save_safe(Some(b"tmp"), None), CONFIG_SUCCESS);
    expect_file(&path, b"[A]\nx=2\n");
    assert!(!scratch.join("safe3.ini.tmp").exists());
    assert!(!scratch.join("safe3.ini.bak").exists());

    c.set_int(b"A", b"x", 3);
    assert_eq!(c.save_safe(Some(b"tmp"), Some(b"")), CONFIG_SUCCESS);
    drop(c);

    expect_file(&path, b"[A]\nx=3\n");
    assert!(!scratch.join("safe3.ini.tmp").exists());
    assert!(!scratch.join("safe3.ini.bak").exists());
}

/// `test_save_safe_invalid_temp_extension`.
#[test]
fn test_save_safe_invalid_temp_extension() {
    let scratch = Scratch::new();
    let path = scratch.join("safe4.ini");
    write_file(&path, b"[A]\nx=1\n");

    let c = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    c.set_int(b"A", b"x", 2);

    assert_eq!(c.save_safe(None, Some(b"bak")), CONFIG_ERROR);
    assert_eq!(c.save_safe(Some(b""), Some(b"bak")), CONFIG_ERROR);
    drop(c);

    expect_file(&path, b"[A]\nx=1\n");
    assert!(!scratch.join("safe4.ini.tmp").exists());
    assert!(!scratch.join("safe4.ini.bak").exists());
}

/* ---------------------------------------------------------------------- */
/* test_config_save.c */

/// `config_save_reports_flush_failure` (`/dev/full` is Linux-only).
#[cfg(target_os = "linux")]
#[test]
fn config_save_reports_flush_failure() {
    let c = Config::create(b"/dev/full").unwrap();
    c.set_string(b"General", b"Name", b"new settings");
    assert_eq!(c.save(), CONFIG_ERROR);
}

/// `config_save_safe_preserves_original` (`/dev/full` is Linux-only).
#[cfg(target_os = "linux")]
#[test]
fn config_save_safe_preserves_original() {
    let scratch = Scratch::new();
    let path = scratch.join("settings.ini");
    let temp = scratch.join("settings.ini.tmp");
    let backup = scratch.join("settings.ini.bak");

    let c = Config::create(&path_bytes(&path)).unwrap();
    c.set_string(b"General", b"Name", b"original settings");
    assert_eq!(c.save(), CONFIG_SUCCESS);
    c.set_string(b"General", b"Name", b"new settings");

    std::os::unix::fs::symlink("/dev/full", &temp).unwrap();
    assert_eq!(c.save_safe(Some(b"tmp"), Some(b"bak")), CONFIG_ERROR);
    drop(c);

    let saved = Config::open(&path_bytes(&path), OpenType::Existing).unwrap();
    assert_eq!(
        saved.get_string(b"General", b"Name").as_deref(),
        Some(b"original settings".as_slice())
    );
    assert!(!backup.exists());
}

/// `config_save_empty_succeeds`.
#[test]
fn config_save_empty_succeeds() {
    let scratch = Scratch::new();
    let path = scratch.join("settings.ini");

    let c = Config::create(&path_bytes(&path)).unwrap();
    assert_eq!(c.save(), CONFIG_SUCCESS);
    assert_eq!(c.save_safe(Some(b"tmp"), None), CONFIG_SUCCESS);
}
