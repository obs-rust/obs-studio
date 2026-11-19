//! #5182: reused CMake tests and Cargo-native cores must share classification data.
use std::path::Path;

pub(crate) fn for_reference(reference: Option<&Path>) -> Result<Vec<String>, String> {
    let Some(reference) = reference else {
        return Ok(Vec::new()); // Standalone builds retain their default empty list.
    };
    let build = reference
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "libobs-c-reference")
        })
        .and_then(Path::parent)
        .ok_or_else(|| format!("not a CMake reference library: {}", reference.display()))?;
    let manifest = build.join("libobs/obs-core-modules.c");
    let source = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    parse(&source).map_err(|error| format!("{}: {error}", manifest.display()))
}

fn parse(source: &str) -> Result<Vec<String>, String> {
    // The shared .c.in emits JSON-compatible quoted target names without a
    // trailing comma, or a lone NULL when count is zero. This is not a C parser.
    let entries = source
        .split_once("const char *obs_core_modules[] = {")
        .and_then(|(_, rest)| rest.split_once("};"))
        .map(|(entries, _)| entries.trim())
        .ok_or("missing generated core-module array")?;
    let names: Vec<String> = if entries == "NULL" {
        Vec::new()
    } else {
        serde_json::from_str(&format!("[{entries}]")).map_err(|error| error.to_string())?
    };
    let count: usize = source
        .split_once("const unsigned int obs_core_modules_count = ")
        .and_then(|(_, rest)| rest.split_once(';'))
        .ok_or("missing generated core-module count")?
        .0
        .trim()
        .parse()
        .map_err(|error| format!("invalid generated core-module count: {error}"))?;
    if names.len() != count {
        return Err(format!(
            "core-module count {count} differs from {} names",
            names.len()
        ));
    }
    Ok(names)
}

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
            core_modules: for_reference(Some(&reference)).unwrap(),
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
    assert!(for_reference(Some(&reference)).unwrap().is_empty());
    assert!(for_reference(None).unwrap().is_empty());
}

#[test]
fn invalid_reference_manifests_fail_instead_of_falling_back_to_defaults() {
    let root = tempfile::tempdir().unwrap();
    let reference = root.path().join("libobs-c-reference/obs.dll");
    assert!(for_reference(Some(&reference)).is_err());
    assert!(for_reference(Some(&root.path().join("obs.dll"))).is_err());
    for (entries, count) in [("\"obs-ffmpeg\"", 2), ("NULL", 1), ("unquoted", 1)] {
        manifest(root.path(), entries, count);
        assert!(for_reference(Some(&reference)).is_err());
    }
    std::fs::write(root.path().join("libobs/obs-core-modules.c"), "truncated").unwrap();
    assert!(for_reference(Some(&reference)).is_err());
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
        let options = options.split_once("\n    };").unwrap().0;
        assert!(
            options.contains("core_modules:")
                && options.contains("core_module_manifest::for_reference(")
                && options.contains(reference),
            "{entry}: Options must use the CMake reference's generated core manifest"
        );
    }
}
