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

#[derive(Default)]
pub struct ApplicationOptions {
    pub browser_panels: bool,
}

/// Build the Linux frontend with the default optional features.
pub fn build_application(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    api: &Path,
    gpu_status_archive: &Path,
) -> Result<PathBuf, String> {
    build_application_with_options(
        build,
        root,
        core,
        api,
        gpu_status_archive,
        &ApplicationOptions::default(),
    )
}

/// Build the Linux frontend graph with its optional browser-panel interface.
#[expect(
    clippy::too_many_lines,
    reason = "flat ordered build stages, kept in one place"
)]
pub fn build_application_with_options(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    api: &Path,
    gpu_status_archive: &Path,
    options: &ApplicationOptions,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let manifest_path = root.join("frontend/build/cargo-sources.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let common_sets: Vec<String> =
        serde_json::from_value(manifest["common_sets"].clone()).map_err(|e| e.to_string())?;
    let mut common = source_set_inputs(&root, &common_sets)?;
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
    if options.browser_panels {
        library.definitions.push(("BROWSER_AVAILABLE".into(), None));
        library
            .include_dirs
            .push(root.join("plugins/obs-browser/panel"));
        let browser_sets: Vec<String> =
            serde_json::from_value(manifest["browser_sets"].clone()).map_err(|e| e.to_string())?;
        common.extend(source_set_inputs(&root, &browser_sets)?);
        common.push("plugins/obs-browser/panel/browser-panel.hpp".into());
    }
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
        // The main window title's dev version (#1983); "dev" without a tag.
        (
            "OBS_RUST_GIT_DESCRIBE".into(),
            Some(serde_json::to_string(&rust_git_describe(&root)).map_err(|e| e.to_string())?),
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
    // CMake resolves generated headers against the binary directory. Resolve
    // those same inputs explicitly in Cargo's frontend configuration directory.
    for input in &mut common {
        if !root.join(&*input).exists() {
            let name = Path::new(&*input)
                .file_name()
                .ok_or("invalid frontend input")?;
            let generated = config.join(name);
            if generated.is_file() {
                *input = generated
                    .to_str()
                    .ok_or("non-UTF8 generated frontend input")?
                    .into();
            }
        }
    }
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
            add_interface_inputs(build, &root, &mut library, &directory, &inputs)?;
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

pub(crate) fn source_set_inputs(root: &Path, keys: &[String]) -> Result<Vec<String>, String> {
    let path = root.join("frontend/build/source-sets.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let sets: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut inputs = Vec::new();
    let mut selected = std::collections::BTreeSet::new();
    for key in keys {
        for input in sets
            .get(key)
            .ok_or_else(|| format!("frontend source set missing: {key}"))?
        {
            let input = format!("frontend/{input}");
            if selected.insert(input.clone()) {
                inputs.push(input);
            }
        }
    }
    Ok(inputs)
}

pub(crate) fn add_interface_inputs(
    build: &NativeBuild,
    root: &Path,
    library: &mut SharedLibrary,
    directory: &str,
    inputs: &[String],
) -> Result<(), String> {
    if directory == "shared/qt/wrappers" {
        let inputs = crate::platform::qt_wrapper_inputs(&build.target, root)?;
        add_inputs(build, root, library, &inputs)
    } else {
        add_inputs(build, root, library, inputs)
    }
}

pub(crate) fn add_inputs(
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

pub(crate) fn write(path: &Path, content: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(content) {
        std::fs::write(path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `git describe` for the newest obs-studio-rust-v* tag, or "" when git or
/// the tag is missing.
fn rust_git_describe(root: &std::path::Path) -> String {
    std::process::Command::new("git")
        .args([
            "describe",
            "--tags",
            "--long",
            "--match",
            "obs-studio-rust-v*",
        ])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|describe| describe.trim().to_owned())
        .unwrap_or_default()
}
