//! Tier 1: `obs_core::nix_system_info`'s distribution, Flatpak and desktop
//! log rules.
//!
//! The `steps_*` cases port `test/cmocka/test_nix_log_rules_steps.c` 1:1:
//! its 8 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_nix_system_info_*` functions; here
//! the safe core is called directly, and the C's copies of the callers'
//! loops are plain byte copies, with a pointer becoming an offset or a range
//! and NULL becoming `None`.
//!
//! One case differs on purpose (#3473): a Flatpak extension entry with an
//! empty name logs no line, where the C logs its commit hash (`=abc` logs
//! `abc`) or `(null)` (`=`). `steps_extension_names` and
//! `steps_extension_lists` assert the port's result there.
//!
//! The `distribution_info_*` cases port `test/cmocka/test_distribution_info.c`
//! 1:1 the same way: its 4 cases, which run `read_distribution_info()` over
//! saved files, under the same names with the same expectations.
//!
//! The `edge_*` cases are new: proptests over any os-release line, value
//! and extension entry from their alphabets, against models written from the
//! C's expressions.

use core::ops::Range;

use obs_core::nix_system_info::{
    DesktopForm, OsReleaseKey, VersionFallback, desktop_form, extension_name, os_release_field,
    os_release_value, version_fallback,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// os-release

fn check_field(line: &str, value: Option<&str>, key: OsReleaseKey) {
    let line = line.as_bytes();
    let got = os_release_field(line);
    match value {
        None => assert_eq!(got, None, "{line:?}"),
        Some(value) => {
            let (got_key, start) = got.expect("a key");
            assert_eq!(got_key, key);
            assert_eq!(start, line.len() - value.len());
            assert_eq!(&line[start..], value.as_bytes());
        }
    }
}

/// Mirrors `os_release_keys`.
#[test]
fn steps_os_release_keys() {
    use OsReleaseKey::{BuildId, Name, VersionId};
    check_field("NAME=Arch\n", Some("Arch\n"), Name);
    check_field("VERSION_ID=\"26.04\"\n", Some("\"26.04\"\n"), VersionId);
    check_field("BUILD_ID=rolling\n", Some("rolling\n"), BuildId);
    check_field("NAME=", Some(""), Name);
    check_field("NAME==x", Some("=x"), Name);

    // compared by prefix and case, from the first byte
    for line in [
        "PRETTY_NAME=Arch\n",
        "NAME =x\n",
        " NAME=x\n",
        "name=x\n",
        "NAME\n",
        "NAM",
        "VERSION_ID\n",
        "VERSION=1\n",
        "BUILD_IDX=1\n",
        "# NAME=x\n",
        "",
    ] {
        check_field(line, None, Name);
    }
}

fn check_value(start: &[u8], expected: &[u8]) {
    let range = os_release_value(start);
    assert!(range.start == 0 || range.start == 1, "{start:?}");
    assert_eq!(&start[range], expected, "{start:?}");
}

/// The bytes before a C string's NUL, as the C reads them.
fn c_str(bytes: &[u8]) -> &[u8] {
    bytes.split(|&b| b == 0).next().unwrap_or_default()
}

/// Mirrors `os_release_values`.
#[test]
fn steps_os_release_values() {
    check_value(b"Arch\n", b"Arch");
    check_value(b"\"Arch Linux\"\n", b"Arch Linux");
    check_value(b"'x'\n", b"x");
    check_value(b"\"26.04\"\r\n", b"26.04");
    check_value(b"plain\r", b"plain");
    check_value(b"last line", b"last line");
    check_value(b"", b"");
    check_value(b"\n", b"");
    check_value(b"\"\"\n", b"");
    check_value(b"''", b"");

    // one byte is not a pair: kept (characterized, not endorsed)
    check_value(b"\"\n", b"\"");
    check_value(b"'", b"'");

    // the quotes must match, and only one pair is cut: no escapes, no shell
    // parse (characterized, not endorsed)
    check_value(b"\"unterminated\n", b"\"unterminated");
    check_value(b"'a\"\n", b"'a\"");
    check_value(b"\"a'\"\n", b"a'");
    check_value(b"\"\"a\"\"\n", b"\"a\"");
    check_value(b"\"a\\\"\"\n", b"a\\\"");
    check_value(b"\"$HOME\"\n", b"$HOME");
    check_value(b"a\"b\"\n", b"a\"b\"");
    check_value(b"\"a\" # x\n", b"\"a\" # x");

    // the span stops at the first CR or LF, and at a NUL (the C string's
    // end, which the shims cut at)
    check_value(b"a\rb\n", b"a");
    check_value(b"\"a\rb\"\n", b"\"a");
    check_value(c_str(b"\"a\0b\"\n"), b"\"a");
}

fn fallback(has_build_id: bool, distro: &str) -> VersionFallback {
    version_fallback(has_build_id, || distro.as_bytes())
}

/// Mirrors `version_fallback`.
#[test]
fn steps_version_fallback() {
    assert_eq!(fallback(true, "Arch Linux"), VersionFallback::BuildId);
    assert_eq!(fallback(true, "Unknown"), VersionFallback::BuildId);
    assert_eq!(fallback(false, "Arch Linux"), VersionFallback::Rolling);
    assert_eq!(fallback(false, "Unknown"), VersionFallback::Unknown);
    assert_eq!(fallback(false, "unknown"), VersionFallback::Rolling);
    assert_eq!(fallback(false, "Unknown "), VersionFallback::Rolling);
    assert_eq!(fallback(false, ""), VersionFallback::Rolling);
}

/// `read_distribution_info()`'s loop over saved text: the lines as
/// `getline()` returns them, each read up to its NUL, and a value copied
/// only when it is not empty.
fn run_distribution(text: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut distro = b"Unknown".to_vec();
    let mut version = Vec::new();
    let mut build_id = Vec::new();
    for line in text.split_inclusive(|&b| b == b'\n') {
        let line = c_str(line);
        let Some((key, start)) = os_release_field(line) else {
            continue;
        };
        let value = &line[start..];
        let range = os_release_value(value);
        if range.is_empty() {
            continue;
        }
        let text = value[range].to_vec();
        match key {
            OsReleaseKey::Name => distro = text,
            OsReleaseKey::VersionId => version = text,
            OsReleaseKey::BuildId => build_id = text,
        }
    }
    if version.is_empty() {
        version = match version_fallback(!build_id.is_empty(), || &distro) {
            VersionFallback::BuildId => build_id,
            VersionFallback::Rolling => b"rolling".to_vec(),
            VersionFallback::Unknown => b"Unknown".to_vec(),
        };
    }
    (distro, version)
}

fn check_distribution(text: &[u8], distro: &str, version: &str) {
    let (got_distro, got_version) = run_distribution(text);
    assert_eq!(got_distro, distro.as_bytes(), "{text:?}");
    assert_eq!(got_version, version.as_bytes(), "{text:?}");
}

/// Mirrors `distribution_files`.
#[test]
fn steps_distribution_files() {
    check_distribution(
        b"NAME=\"Arch Linux\"\nBUILD_ID=rolling\n",
        "Arch Linux",
        "rolling",
    );
    check_distribution(b"NAME='x'\n", "x", "rolling");
    check_distribution(b"NAME=\"unterminated\n", "\"unterminated", "rolling");
    check_distribution(
        b"NAME=Ubuntu\r\nVERSION_ID=\"26.04\"\r\n",
        "Ubuntu",
        "26.04",
    );
    check_distribution(b"NAME=Fedora\nVERSION_ID=43", "Fedora", "43");

    // an empty value is not copied
    check_distribution(b"NAME=\n", "Unknown", "Unknown");
    check_distribution(b"NAME=\"\"\n", "Unknown", "Unknown");
    // one quote is a value (characterized, not endorsed)
    check_distribution(b"NAME=\"\n", "\"", "rolling");

    // keys that don't match
    check_distribution(
        b"PRETTY_NAME=\"Arch Linux\"\nNAME =x\n",
        "Unknown",
        "Unknown",
    );

    // a later key replaces an earlier one, unless it is empty
    check_distribution(
        b"NAME=a\nVERSION_ID=1\nVERSION_ID=2\nVERSION_ID=\n",
        "a",
        "2",
    );
    check_distribution(b"NAME=a\nNAME=b\nNAME=''\n", "b", "rolling");

    // the fallback
    check_distribution(b"NAME=Arch\nBUILD_ID=20261013\n", "Arch", "20261013");
    check_distribution(b"BUILD_ID=b\n", "Unknown", "b");
    check_distribution(b"NAME=Arch\n", "Arch", "rolling");
    check_distribution(b"", "Unknown", "Unknown");
    // NAME=Unknown in the file reads as no name (characterized, not
    // endorsed)
    check_distribution(b"NAME=Unknown\n", "Unknown", "Unknown");

    // a NUL inside a line ends it for every check
    check_distribution(b"NAME=a\0b\nVERSION_ID=\"1\0\"\n", "a", "\"1");
    check_distribution(b"\0NAME=a\n", "Unknown", "Unknown");
}

/// Mirrors `test_distribution_info`'s `arch_build_id`.
#[test]
fn distribution_info_arch_build_id() {
    check_distribution(
        b"NAME=\"Arch Linux\"\nBUILD_ID=rolling\n",
        "Arch Linux",
        "rolling",
    );
}

/// Mirrors `test_distribution_info`'s `rolling_without_build_id`.
#[test]
fn distribution_info_rolling_without_build_id() {
    check_distribution(b"NAME='Arch Linux'\n", "Arch Linux", "rolling");
}

/// Mirrors `test_distribution_info`'s `version_precedes_build_id`.
#[test]
fn distribution_info_version_precedes_build_id() {
    check_distribution(
        b"NAME=Ubuntu\nVERSION_ID=\"26.04\"\nBUILD_ID=123\n",
        "Ubuntu",
        "26.04",
    );
    check_distribution(
        b"BUILD_ID=123\nNAME=Ubuntu\nVERSION_ID=26.04",
        "Ubuntu",
        "26.04",
    );
}

/// Mirrors `test_distribution_info`'s `exact_keys_and_empty_values`.
#[test]
fn distribution_info_exact_keys_and_empty_values() {
    check_distribution(
        b"NAME_VARIANT=wrong\nNAME=Arch\nVERSION_ID=\"\"\nBUILD_ID=\"20261013\"\n",
        "Arch",
        "20261013",
    );
    check_distribution(
        b"# NAME=wrong\nNAME=\nVERSION_ID_SUFFIX=wrong\n",
        "Unknown",
        "Unknown",
    );
}

// ---------------------------------------------------------------------------
// Flatpak extensions

fn name(ext: &str) -> Option<&str> {
    extension_name(ext.as_bytes()).map(|range| &ext[range])
}

/// Mirrors `extension_names`.
#[test]
fn steps_extension_names() {
    assert_eq!(
        name("org.freedesktop.Platform.GL.default=4c1f8b2a"),
        Some("org.freedesktop.Platform.GL.default")
    );
    assert_eq!(name("a=1"), Some("a"));
    assert_eq!(name("a"), Some("a"));
    assert_eq!(name("a="), Some("a"));
    assert_eq!(name("a==b"), Some("a"));
    assert_eq!(name("a=b=c"), Some("a"));

    // An empty name logs no line (#3473): the C logs the next non-empty
    // piece, the commit hash ("abc").
    assert_eq!(name("=abc"), None);
    assert_eq!(name("==abc=d"), None);

    // No piece at all logs no line either (#3473): the C logs "(null)".
    assert_eq!(name("="), None);
    assert_eq!(name("=="), None);
    assert_eq!(name(""), None);
}

/// `log_flatpak_extensions()`'s loop: the `;` split, which drops empty
/// entries, then one line per entry with a name.
fn run_extensions(extensions: &str) -> Vec<String> {
    extensions
        .split(';')
        .filter(|ext| !ext.is_empty())
        .filter_map(name)
        .map(|name| format!(" - {name}"))
        .collect()
}

/// Mirrors `extension_lists`.
#[test]
fn steps_extension_lists() {
    let ab = [" - a", " - b"];
    assert_eq!(run_extensions("a=1;b=2"), ab);
    assert_eq!(run_extensions("a;b"), ab);
    assert_eq!(run_extensions("a=1;;b=2"), ab);
    assert_eq!(run_extensions(";a=1;b=2;"), ab);
    assert_eq!(run_extensions("a=1;"), [" - a"]);

    // an empty name logs no line (#3473): the C logs " - abc" and
    // " - (null)"
    assert_eq!(run_extensions("a=1;=abc"), [" - a"]);
    assert_eq!(run_extensions("=;b=2"), [" - b"]);

    // an empty value logs the heading and no entries (characterized)
    assert!(run_extensions("").is_empty());
    assert!(run_extensions(";;").is_empty());
}

// ---------------------------------------------------------------------------
// desktop

/// Mirrors `desktop_forms`.
#[test]
fn steps_desktop_forms() {
    assert_eq!(desktop_form(true, true), DesktopForm::Both);
    assert_eq!(desktop_form(true, false), DesktopForm::One);
    assert_eq!(desktop_form(false, true), DesktopForm::One);
    assert_eq!(desktop_form(false, false), DesktopForm::None);
}

/// `log_desktop_session_info()`'s line, from the form.
fn desktop_line(current: Option<&str>, session: Option<&str>) -> String {
    match desktop_form(current.is_some(), session.is_some()) {
        DesktopForm::Both => format!(
            "Desktop Environment: {} ({})",
            current.unwrap_or_default(),
            session.unwrap_or_default()
        ),
        DesktopForm::One => format!(
            "Desktop Environment: {}",
            current.or(session).unwrap_or_default()
        ),
        DesktopForm::None => String::new(),
    }
}

/// Mirrors `desktop_lines`.
#[test]
fn steps_desktop_lines() {
    let check = |current, session, expected: &str| {
        assert_eq!(desktop_line(current, session), expected);
    };
    check(
        Some("GNOME"),
        Some("gnome"),
        "Desktop Environment: GNOME (gnome)",
    );
    check(Some("KDE"), None, "Desktop Environment: KDE");
    check(None, Some("plasma"), "Desktop Environment: plasma");
    check(None, None, "");

    // an empty value counts as set (characterized, not endorsed)
    check(Some(""), Some(""), "Desktop Environment:  ()");
    check(Some(""), None, "Desktop Environment: ");
    check(None, Some(""), "Desktop Environment: ");
    check(Some(""), Some("gnome"), "Desktop Environment:  (gnome)");
    check(Some("GNOME"), Some(""), "Desktop Environment: GNOME ()");
}

// ---------------------------------------------------------------------------
// models and proptests

/// The C's key checks: `strncmp(line, key, strlen(key))` in order.
fn model_field(line: &[u8]) -> Option<(OsReleaseKey, usize)> {
    [
        (&b"NAME="[..], OsReleaseKey::Name),
        (b"VERSION_ID=", OsReleaseKey::VersionId),
        (b"BUILD_ID=", OsReleaseKey::BuildId),
    ]
    .into_iter()
    .find(|(key, _)| line.starts_with(key))
    .map(|(key, which)| (which, key.len()))
}

/// The C's `strcspn(start, "\r\n")` and its quote cut.
fn model_value(start: &[u8]) -> Range<usize> {
    let len = start
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(start.len());
    if len >= 2 && (start[0] == b'"' || start[0] == b'\'') && start[len - 1] == start[0] {
        1..len - 1
    } else {
        0..len
    }
}

fn os_release_line() -> impl Strategy<Value = Vec<u8>> {
    let pieces = prop::sample::select(vec![
        &b"NAME="[..],
        b"VERSION_ID=",
        b"BUILD_ID=",
        b"NAME",
        b"NAM",
        b"VERSION_ID",
        b"BUILD_ID",
        b"PRETTY_NAME=",
        b" ",
        b"=",
        b"\"",
        b"'",
        b"\r",
        b"\n",
        b"\\",
        b"$",
        b"a",
        b"Unknown",
        b"\xc3\xa9",
        b"\xff",
    ]);
    prop::collection::vec(pieces, 0..8).prop_map(|pieces| pieces.concat())
}

fn extension_entry() -> impl Strategy<Value = Vec<u8>> {
    let pieces = prop::sample::select(vec![
        &b"="[..],
        b"==",
        b"a",
        b"org.gtk.Gtk3theme",
        b"4c1f8b2a",
        b" ",
        b".",
        b"\xc3\xa9",
        b"\xff",
    ]);
    prop::collection::vec(pieces, 0..8).prop_map(|pieces| pieces.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Any line holds the key the C's three prefix checks give, and its
    /// value starts right after it.
    #[test]
    fn edge_os_release_field(line in os_release_line()) {
        prop_assert_eq!(os_release_field(&line), model_field(&line));
    }

    /// Any value's span and quote cut is the C's, inside the value.
    #[test]
    fn edge_os_release_value(start in os_release_line()) {
        let range = os_release_value(&start);
        prop_assert!(range.end <= start.len());
        prop_assert_eq!(range, model_value(&start));
    }

    /// The fallback reads the distro only without a BUILD_ID, and is
    /// "Unknown" only for exactly that name.
    #[test]
    fn edge_version_fallback(has_build_id: bool, distro in os_release_line()) {
        let mut read = false;
        let got = version_fallback(has_build_id, || {
            read = true;
            &distro
        });
        prop_assert_eq!(read, !has_build_id);
        let expected = if has_build_id {
            VersionFallback::BuildId
        } else if distro == b"Unknown" {
            VersionFallback::Unknown
        } else {
            VersionFallback::Rolling
        };
        prop_assert_eq!(got, expected);
    }

    /// Any entry's name is the bytes before its first '=', or none.
    #[test]
    fn edge_extension_name(ext in extension_entry()) {
        let end = ext.iter().position(|&b| b == b'=').unwrap_or(ext.len());
        prop_assert_eq!(extension_name(&ext), (end != 0).then_some(0..end));
    }
}
