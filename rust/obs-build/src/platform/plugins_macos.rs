//! Native macOS capture language settings and framework dependencies.
use crate::{
    LibraryKind, NativeBuild, SharedLibrary, SourceCompileOptions,
    libobs::Options,
    plugins::{MacosCaptureModule, Module},
};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

fn resource_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<(), String> {
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name() == ".DS_Store" {
            continue;
        }
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            resource_files(root, &path, files)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
            files.push((
                path.strip_prefix(root).map_err(|e| e.to_string())?.into(),
                std::fs::read(path).map_err(|e| e.to_string())?,
            ));
        }
    }
    Ok(())
}

fn stage_bundle(binary: &Path, data: &Path, name: &str) -> Result<Module, String> {
    let image = std::fs::read(binary).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    resource_files(data, data, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    // This development bundle supplies runtime discovery metadata only. Release
    // versioning, signing and distribution packaging remain separate inputs.
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>{name}</string>
<key>CFBundleIdentifier</key><string>com.obsproject.{name}</string>
<key>CFBundleName</key><string>{name}</string>
<key>CFBundlePackageType</key><string>BNDL</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
</dict></plist>
"#
    );
    // Select a snapshot of all runtime inputs, so removed resources cannot be
    // picked up from a previously staged same-named module.
    let mut signature = std::collections::hash_map::DefaultHasher::new();
    (&image, &files, &plist).hash(&mut signature);
    let contents = binary
        .parent()
        .ok_or("capture binary has no parent")?
        .join("bundles")
        .join(format!("{:016x}", signature.finish()))
        .join(format!("{name}.plugin/Contents"));
    let library = contents.join("MacOS").join(name);
    let resources = contents.join("Resources");
    std::fs::create_dir_all(library.parent().ok_or("bundle binary has no parent")?)
        .map_err(|e| e.to_string())?;
    std::fs::write(&library, image).map_err(|e| e.to_string())?;
    std::fs::set_permissions(
        &library,
        std::fs::metadata(binary)
            .map_err(|e| e.to_string())?
            .permissions(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&resources).map_err(|e| e.to_string())?;
    for (relative, bytes) in files {
        let destination = resources.join(relative);
        std::fs::create_dir_all(destination.parent().ok_or("resource has no parent")?)
            .map_err(|e| e.to_string())?;
        std::fs::write(destination, bytes).map_err(|e| e.to_string())?;
    }
    std::fs::write(contents.join("Info.plist"), plist).map_err(|e| e.to_string())?;
    Ok(Module {
        library,
        data: resources,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one block per captured plugin output"
)]
pub fn capture(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &Options,
    module: MacosCaptureModule,
) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let manifest = root.join("plugins/mac-capture/source-sets.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let sets: BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&std::fs::read(manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let key = module.name().replace('-', "_").to_uppercase();
    let sources = sets
        .get(&key)
        .ok_or_else(|| format!("missing capture source set {key}"))?;
    let directory = match module {
        MacosCaptureModule::ScreenAudio => "mac-capture",
        MacosCaptureModule::Camera | MacosCaptureModule::LegacyCamera => "mac-avcapture",
    };
    let dependencies = options
        .native_dependencies
        .as_ref()
        .ok_or("macOS capture requires an explicit OBS dependency bundle")?;
    let include = dependencies
        .join("include")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !include.join("simde/x86/sse2.h").is_file() {
        return Err("macOS capture dependency bundle lacks simde".into());
    }
    let mut library = SharedLibrary {
        name: module.name().into(),
        kind: LibraryKind::Module,
        file_name: Some(module.name().into()),
        sources: sources.iter().map(|path| root.join(path)).collect(),
        include_dirs: vec![
            root.clone(),
            root.join("libobs"),
            root.join("plugins").join(directory),
            build.output.join("config"),
            include,
        ],
        dependencies: vec![core.canonicalize().map_err(|e| e.to_string())?],
        compile_options: [
            "-fno-strict-aliasing",
            "-fvisibility=hidden",
            "-mmacosx-version-min=13.0",
        ]
        .map(String::from)
        .into(),
        c_compile_options: vec!["-std=gnu17".into()],
        cpp_compile_options: vec!["-std=gnu++17".into()],
        ..SharedLibrary::default()
    };
    let objc = library
        .sources
        .iter()
        .filter(|source| {
            matches!(
                source.extension().and_then(|value| value.to_str()),
                Some("m" | "mm")
            )
        })
        .cloned()
        .collect();
    let objc_options = match module {
        MacosCaptureModule::ScreenAudio => vec!["-fno-objc-arc".into()],
        MacosCaptureModule::Camera => vec!["-fobjc-arc".into(), "-fmodules".into()],
        MacosCaptureModule::LegacyCamera => vec!["-fobjc-arc".into()],
    };
    library.source_compile_options.push(SourceCompileOptions {
        sources: objc,
        options: objc_options,
    });
    let frameworks: &[&str] = match module {
        MacosCaptureModule::ScreenAudio => &[
            "AudioUnit",
            "Cocoa",
            "CoreAudio",
            "CoreFoundation",
            "CoreMedia",
            "CoreVideo",
            "IOSurface",
        ],
        MacosCaptureModule::Camera | MacosCaptureModule::LegacyCamera => &[
            "AVFoundation",
            "Cocoa",
            "CoreFoundation",
            "CoreMedia",
            "CoreVideo",
            "CoreMediaIO",
            "IOSurface",
        ],
    };
    for framework in frameworks {
        library
            .link_options
            .extend(["-framework".into(), (*framework).into()]);
    }
    if matches!(module, MacosCaptureModule::ScreenAudio) {
        library
            .link_options
            .extend(["-weak_framework".into(), "ScreenCaptureKit".into()]);
    }
    if matches!(module, MacosCaptureModule::LegacyCamera) {
        library
            .include_dirs
            .push(root.join("plugins/mac-avcapture/legacy"));
    }
    if !options.unity {
        build.separate_target_sources(&root, &mut library)?;
    }
    if options.pch {
        library.precompiled_headers = crate::groups::headers(&root, &library.sources)?;
    }
    stage_bundle(
        &build.shared_library(&library)?,
        &root.join("plugins").join(directory).join("data"),
        module.name(),
    )
}
