//! #5182: reused CMake tests and Cargo-native cores must share classification data.

#[path = "../libobs/core_module_manifest.rs"]
mod driver;

fn manifest(root: &std::path::Path, entries: &str, count: usize) {
    let source = include_str!("../../../../libobs/obs-core-modules.c.in")
        .replace("@OBS_CORE_MODULE_LIST@", entries)
        .replace("@OBS_CORE_MODULE_COUNT@", &count.to_string());
    std::fs::create_dir_all(root.join("libobs")).unwrap();
    std::fs::write(root.join("libobs/obs-core-modules.c"), source).unwrap();
}

#[test]
fn both_reference_layouts_reuse_generated_names_verbatim() {
    let root = tempfile::tempdir().unwrap();
    let names = ["obs-ffmpeg", "CoreMixed", "core.plugin"];
    let entries = names
        .map(|name| serde_json::to_string(name).unwrap())
        .join(",\n");
    manifest(root.path(), &entries, names.len());
    for layout in [
        "RelWithDebInfo/obs.dll",
        "RelWithDebInfo/libobs.framework/Versions/A/libobs",
    ] {
        let reference = root.path().join("libobs-c-reference").join(layout);
        let options = obs_build::libobs::Options {
            core_modules: driver::for_reference(Some(&reference)).unwrap(),
            ..Default::default()
        };
        assert_eq!(options.core_modules, names);
    }
}

#[test]
fn empty_reference_and_standalone_defaults_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    manifest(root.path(), "NULL", 0);
    let reference = root.path().join("libobs-c-reference/obs.dll");
    assert!(driver::for_reference(Some(&reference)).unwrap().is_empty());
    assert!(driver::for_reference(None).unwrap().is_empty());
}

#[test]
fn invalid_reference_manifests_fail_instead_of_falling_back_to_defaults() {
    let root = tempfile::tempdir().unwrap();
    let reference = root.path().join("libobs-c-reference/obs.dll");
    assert!(driver::for_reference(Some(&reference)).is_err());
    assert!(driver::for_reference(Some(&root.path().join("obs.dll"))).is_err());
    for (entries, count) in [("\"obs-ffmpeg\"", 2), ("NULL", 1), ("unquoted", 1)] {
        manifest(root.path(), entries, count);
        assert!(driver::for_reference(Some(&reference)).is_err());
    }
    std::fs::write(root.path().join("libobs/obs-core-modules.c"), "truncated").unwrap();
    assert!(driver::for_reference(Some(&reference)).is_err());
}

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
        let options = body
            .split_once("let options = libobs::Options {")
            .unwrap()
            .1;
        let options = options
            .split_once("..libobs::Options::default()")
            .unwrap()
            .0;
        assert!(
            options.contains("core_modules:")
                && options.contains("core_module_manifest::for_reference(")
                && options.contains(reference),
            "{entry}: Options must use the CMake reference's generated core manifest"
        );
    }
}
