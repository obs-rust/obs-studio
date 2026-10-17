//! Native frontend support libraries and application build graph.
use crate::{NativeBuild, SharedLibrary, libobs::add_package};
use std::path::{Path, PathBuf};

pub fn build_api(build: &NativeBuild, root: &Path, core: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = SharedLibrary {
        name: "obs-frontend-api".into(),
        sources: vec![root.join("frontend/api/build/obs-frontend-api.core+.hpp.cpp")],
        include_dirs: vec![
            root.clone(),
            root.join("libobs"),
            root.join("frontend/api"),
            build.output.join("config"),
        ],
        dependencies: vec![core.canonicalize().map_err(|e| e.to_string())?],
        ..SharedLibrary::default()
    };
    crate::platform::configure_frontend_api(&build.target, &mut library)?;
    add_package(&mut library, &build.target, "simde")?;
    let artifact = build.shared_library(&library)?;
    let lookup = artifact.with_file_name(crate::platform::shared_filename(
        &build.target,
        &library.name,
    ));
    if artifact != lookup {
        std::fs::copy(&artifact, lookup).map_err(|e| e.to_string())?;
    }
    Ok(artifact)
}

/// Add the shared Qt spinbox interface sources to a native target.
pub fn add_spinbox(
    build: &NativeBuild,
    root: &Path,
    library: &mut SharedLibrary,
) -> Result<(), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    crate::platform::configure_cpp17(&build.target, library);
    let directory = root.join("shared/qt/obs-spinbox");
    library
        .sources
        .push(directory.join("build/qt-obs-spinbox.core+.hpp.cpp"));
    library.include_dirs.push(directory.clone());
    add_package(library, &build.target, "Qt6Widgets")?;
    for name in ["AutoSelectLineEdit", "DoubleSpinBox", "SpinBox"] {
        crate::qt::moc(
            library,
            &directory.join(format!("{name}.hpp")),
            &build
                .output
                .join(&library.name)
                .join(format!("qt/moc_{name}.cpp")),
        )?;
    }
    Ok(())
}

/// Build the Linux frontend graph without browser panels; broader platform and
/// feature configurations remain under conversion.
pub fn build_application(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    api: &Path,
    gpu_status_archive: &Path,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let manifest_path = root.join("frontend/build/cargo-sources.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let common: Vec<String> =
        serde_json::from_value(manifest["common"].clone()).map_err(|e| e.to_string())?;
    let interfaces: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_value(manifest["interfaces"].clone()).map_err(|e| e.to_string())?;
    let config = build.output.join("frontend-config");
    std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
    let mut library = SharedLibrary {
        name: "obs-studio-rust".into(),
        include_dirs: vec![
            config.clone(),
            root.clone(),
            root.join("frontend"),
            root.join("frontend/api"),
            root.join("libobs"),
            root.join("shared/bpm"),
            root.join("deps/json11"),
            build.output.join("config"),
        ],
        dependencies: vec![
            core.canonicalize().map_err(|e| e.to_string())?,
            api.canonicalize().map_err(|e| e.to_string())?,
        ],
        ..SharedLibrary::default()
    };
    crate::platform::configure_application(&build.target, &root, &manifest, &config, &mut library)?;
    for package in [
        "simde",
        "Qt6Widgets",
        "Qt6Network",
        "Qt6Svg",
        "Qt6Xml",
        "libcurl",
        "libavcodec",
        "libavutil",
        "libavformat",
        "nlohmann_json",
    ] {
        add_package(&mut library, &build.target, package)?;
    }
    let version_manifest = std::fs::read_to_string(root.join("rust/libobs-rust/Cargo.toml"))
        .map_err(|e| e.to_string())?;
    let version = version_manifest
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = \"")
                .and_then(|line| line.strip_suffix('"'))
        })
        .ok_or("libobs-rust package version missing")?;
    library.definitions.extend([
        (
            "OBS_RUST_VERSION".into(),
            Some(serde_json::to_string(version).map_err(|e| e.to_string())?),
        ),
        ("OBS_GPU_STATUS".into(), Some("1".into())),
        (
            "QT_ENABLE_STRICT_MODE_UP_TO".into(),
            Some("0xFF0000".into()),
        ),
    ]);
    let mut ui_config =
        std::fs::read_to_string(root.join("frontend/cmake/templates/ui-config.h.in"))
            .map_err(|e| e.to_string())?;
    for (name, value) in [
        ("OAUTH_BASE_URL", "https://auth.obsproject.com/"),
        ("TWITCH_CLIENTID", ""),
        ("TWITCH_HASH", "0"),
        ("RESTREAM_CLIENTID", ""),
        ("RESTREAM_HASH", "0"),
        ("YOUTUBE_CLIENTID", ""),
        ("YOUTUBE_SECRET", ""),
        ("YOUTUBE_CLIENTID_HASH", "0"),
        ("YOUTUBE_SECRET_HASH", "0"),
    ] {
        ui_config = ui_config.replace(&format!("@{name}@"), value);
    }
    write(&config.join("ui-config.h"), &ui_config)?;
    for directory in interfaces.keys() {
        let path = root.join(directory);
        library
            .include_dirs
            .push(if directory == "shared/qt/idian" {
                path.join("include")
            } else {
                path
            });
    }
    library
        .include_dirs
        .push(root.join("shared/qt/obs-spinbox"));
    add_inputs(build, &root, &mut library, &common)?;
    add_spinbox(build, &root, &mut library)?;
    for (directory, inputs) in interfaces {
        let path = root.join(&directory);
        if directory == "shared/qt/idian" {
            let mut idian = SharedLibrary {
                name: "obs-idian".into(),
                include_dirs: vec![root.clone(), path.join("include")],
                ..SharedLibrary::default()
            };
            crate::platform::configure_cpp_module(&build.target, &mut idian)?;
            for package in ["Qt6Widgets", "Qt6Svg"] {
                add_package(&mut idian, &build.target, package)?;
            }
            add_inputs(build, &root, &mut idian, &inputs)?;
            crate::qt::combine_moc(
                &mut idian,
                &build.output.join("obs-idian/qt/mocs_compilation.cpp"),
                &root.join("shared/qt/idian/build/idian.pch.hpp"),
            )?;
            library.dependencies.push(build.static_library(&idian)?);
            library.include_dirs.push(path.join("include"));
        } else {
            library.include_dirs.push(path);
            add_inputs(build, &root, &mut library, &inputs)?;
        }
    }
    library.sources.push(root.join("deps/json11/json11.cpp"));
    library
        .dependencies
        .push(crate::deps::shared_archive(build, &root, "bpm")?);
    library
        .dependencies
        .push(crate::libobs::build_caption(build, &root)?);
    library.dependencies.push(
        gpu_status_archive
            .canonicalize()
            .map_err(|e| e.to_string())?,
    );
    crate::qt::combine_moc(
        &mut library,
        &build.output.join("obs-studio-rust/qt/mocs_compilation.cpp"),
        &root.join("frontend/build/frontend-components.pch.hpp"),
    )?;
    build.executable(&library)
}

fn add_inputs(
    build: &NativeBuild,
    root: &Path,
    library: &mut SharedLibrary,
    inputs: &[String],
) -> Result<(), String> {
    let generated = build.output.join(&library.name).join("qt");
    // All generated UI headers must exist before moc preprocesses widget headers.
    for input in inputs
        .iter()
        .filter(|input| input.ends_with(".ui") || input.ends_with(".qrc"))
    {
        let path = root.join(input);
        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("invalid Qt input name")?;
        if input.ends_with(".ui") {
            crate::qt::uic(library, &path, &generated.join(format!("ui_{stem}.h")))?;
        } else {
            crate::qt::rcc(library, &path, &generated.join(format!("qrc_{stem}.cpp")))?;
        }
    }
    for input in inputs {
        let path = root.join(input);
        if input.ends_with(".c") || input.ends_with(".cpp") {
            library.sources.push(path);
        } else if input.ends_with(".hpp") || input.ends_with(".h") {
            let header =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if header.lines().any(|line| {
                ["Q_OBJECT", "Q_GADGET", "Q_NAMESPACE"]
                    .iter()
                    .any(|keyword| line.trim_start().starts_with(keyword))
            }) {
                let name = input.replace(['/', '.', '-'], "_");
                crate::qt::moc(library, &path, &generated.join(format!("moc_{name}.cpp")))?;
            }
        }
    }
    Ok(())
}

fn write(path: &Path, content: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(content) {
        std::fs::write(path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}
