//! When obs-util's build script probes gio-2.0 (#1576). The Rust session
//! D-Bus inhibitor (`ffi::dbus_inhibit`) declares GIO's functions itself, so
//! the probe only supplies the link flags. Shared with its unit test.

/// What the build script does with gio-2.0.
#[derive(Debug, PartialEq, Eq)]
pub enum GioProbe {
    /// No GIO on this target: no `gio` feature, not Unix, or macOS.
    Skip,
    /// Probe it and fail the build without it.
    Require,
}

/// `feature`: the `gio` feature is on. `host` and `target`: Cargo's triples.
pub fn gio_probe(
    feature: bool,
    unix: bool,
    target_os: &str,
    _host: &str,
    _target: &str,
) -> GioProbe {
    if feature && unix && target_os != "macos" {
        GioProbe::Require
    } else {
        GioProbe::Skip
    }
}
