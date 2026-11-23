//! Linux VST3 audio filter and its out-of-process scanner.
use super::{Module, finish, module};
use crate::{NativeBuild, SharedLibrary, libobs::add_package, platform};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Artifacts {
    pub module: Module,
    pub scanner: PathBuf,
}

#[derive(serde::Deserialize)]
struct SdkSources {
    common: Vec<PathBuf>,
    linux: Vec<PathBuf>,
}

pub fn build(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    api: &Path,
    sdk: &Path,
    scanner: &Path,
) -> Result<Artifacts, String> {
    platform::validate_vst3_target(&build.target)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let sdk = sdk
        .canonicalize()
        .map_err(|e| format!("VST3 SDK {}: {e}", sdk.display()))?;
    let sdk_include = sdk.to_str().ok_or("VST3 SDK path is not UTF-8")?;
    let source = root.join("plugins/obs-vst3");
    let manifest = source.join("build/sdk-source-sets.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let sources: SdkSources = serde_json::from_slice(
        &std::fs::read(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?,
    )
    .map_err(|e| format!("{}: {e}", manifest.display()))?;
    let sdk_sources = sources
        .common
        .into_iter()
        .chain(sources.linux)
        .map(|path| sdk.join(path))
        .collect::<Vec<_>>();
    for path in &sdk_sources {
        if !path.is_file() {
            return Err(format!("VST3 SDK source is missing: {}", path.display()));
        }
    }
    let mut sdk_library = SharedLibrary {
        name: "steinberg-vst3".into(),
        sources: sdk_sources,
        compile_options: vec!["-fvisibility=hidden".into()],
        ..SharedLibrary::default()
    };
    platform::configure_vst3(&mut sdk_library, sdk_include);
    let sdk_archive = build.static_library(&sdk_library)?;

    let discovery = source.join("build/obs-vst3.discovery+.hpp.cpp");
    let discovery_host = source.join("build/obs-vst3.discovery.linux+.hpp.cpp");
    let mut library = module(build, &root, core, "obs-vst3")?;
    library.sources = vec![
        source.join("build/obs-vst3.core+.hpp.cpp"),
        source.join("build/obs-vst3.editor+.hpp.cpp"),
        source.join("build/obs-vst3.editor.linux+.hpp.cpp"),
        source.join("build/obs-vst3.linux+.hpp.cpp"),
        discovery,
        discovery_host,
    ];
    library.include_dirs.push(root.join("frontend/api"));
    library
        .dependencies
        .extend([api.canonicalize().map_err(|e| e.to_string())?, sdk_archive]);
    platform::configure_vst3(&mut library, sdk_include);
    add_package(&mut library, &build.target, "Qt6Widgets")?;
    crate::qt::moc(
        &mut library,
        &source.join("VST3Plugin.h"),
        &build.output.join("obs-vst3/qt/moc_VST3Plugin.cpp"),
    )?;

    let scanner = scanner
        .canonicalize()
        .map_err(|error| format!("VST3 scanner {}: {error}", scanner.display()))?;
    Ok(Artifacts {
        module: finish(build, &root, library)?,
        scanner,
    })
}
