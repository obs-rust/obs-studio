//! #5182: reused CMake tests and Cargo-native cores must share classification data.

#[test]
fn native_driver_options_preserve_reference_core_modules() {
    for (source, entry, reference) in [
        (
            include_str!("../libobs.rs"),
            "fn macos_core_builds_from_the_canonical_manifest()",
            "OBS_BUILD_MACOS_REFERENCE",
        ),
        (
            include_str!("../libobs/windows.rs"),
            "fn core_builds_from_the_canonical_manifest()",
            "OBS_BUILD_WINDOWS_REFERENCE",
        ),
    ] {
        let body = source.split_once(entry).unwrap().1;
        let options = body.split_once("let options = libobs::Options {").unwrap().1;
        let options = options.split_once("..libobs::Options::default()").unwrap().0;
        assert!(
            options.contains("core_modules:")
                && options.contains("core_module_manifest::for_reference(")
                && options.contains(reference),
            "{entry}: Options must use the CMake reference's generated core manifest"
        );
    }
}
