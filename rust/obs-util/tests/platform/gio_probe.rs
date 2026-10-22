//! Tier 1: when obs-util's build script needs gio-2.0 (#1576). Host-neutral.
//!
//! A native Linux build links the test binaries against GIO, so it must
//! find it. Cross builds only lint or build the staticlib, which CMake links
//! with GIO itself; a target sysroot without GIO must not fail them.

use super::build_gio::{GioProbe, gio_probe};

const X64: &str = "x86_64-unknown-linux-gnu";
const ARM64: &str = "aarch64-unknown-linux-gnu";

#[test]
fn native_linux_requires_gio() {
    assert_eq!(gio_probe(true, true, "linux", X64, X64), GioProbe::Require);
    assert_eq!(
        gio_probe(true, true, "linux", ARM64, ARM64),
        GioProbe::Require
    );
}

#[test]
fn cross_linux_does_not_require_gio() {
    assert_ne!(
        gio_probe(true, true, "linux", X64, ARM64),
        GioProbe::Require
    );
    assert_ne!(
        gio_probe(true, true, "freebsd", X64, "x86_64-unknown-freebsd"),
        GioProbe::Require
    );
}

#[test]
fn no_gio_without_the_feature_or_off_linux_and_freebsd() {
    assert_eq!(gio_probe(false, true, "linux", X64, X64), GioProbe::Skip);
    assert_eq!(
        gio_probe(
            true,
            true,
            "macos",
            "aarch64-apple-darwin",
            "aarch64-apple-darwin"
        ),
        GioProbe::Skip
    );
    assert_eq!(
        gio_probe(true, false, "windows", X64, "x86_64-pc-windows-msvc"),
        GioProbe::Skip
    );
}
