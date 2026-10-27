//! The native libobs graph, shared by bundled obs-sys and the application build.
use crate::{NativeBuild, SharedLibrary};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Options {
    /// Keep explicit BUILD.md unity groups; false compiles their members separately.
    pub unity: bool,
    /// Compile the explicit group headers; independent of the unity option.
    pub pch: bool,
    pub hevc: bool,
    pub wayland: bool,
    pub pulseaudio: bool,
    pub gio: bool,
    pub core_modules: Vec<String>,
    /// Prebuilt OBS dependency bundle for targets without pkg-config metadata.
    pub native_dependencies: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            unity: true,
            pch: false,
            hevc: true,
            wayland: true,
            pulseaudio: true,
            gio: true,
            core_modules: vec![],
            native_dependencies: None,
        }
    }
}

/// Build production hybrid libobs using the externally built release Rust archive.
pub fn build(build: &NativeBuild, root: &Path) -> Result<PathBuf, String> {
    build_with_options(build, root, &Options::default())
}

pub fn build_with_options(
    build: &NativeBuild,
    root: &Path,
    options: &Options,
) -> Result<PathBuf, String> {
    let archive = std::env::var_os("OBS_BUILD_RUST_ARCHIVE")
        .map(PathBuf::from)
        .ok_or("set OBS_BUILD_RUST_ARCHIVE to the release libobs-rust staticlib; C rollback uses build_c_reference_with_options explicitly")?;
    build_hybrid_with_options(build, root, options, &archive)
}

/// Explicit C baseline for migration testing and temporary rollback.
pub fn build_c_reference_with_options(
    build: &NativeBuild,
    root: &Path,
    options: &Options,
) -> Result<PathBuf, String> {
    let plan = prepare(build, root, options)?;
    let mut library = plan.common;
    add_variant(build, &plan.root, &plan.sets, options, &mut library, None)?;
    finish(build, &library)
}

/// Build only the production hybrid; never compile its replaced C sources.
pub fn build_hybrid_with_options(
    build: &NativeBuild,
    root: &Path,
    options: &Options,
    rust_archive: &Path,
) -> Result<PathBuf, String> {
    if !rust_archive.is_file() {
        return Err(format!(
            "Rust archive does not exist: {}",
            rust_archive.display()
        ));
    }
    let plan = prepare(build, root, options)?;
    let mut library = plan.common;
    add_variant(
        build,
        &plan.root,
        &plan.sets,
        options,
        &mut library,
        Some(rust_archive),
    )?;
    finish(build, &library)
}

struct Plan {
    common: SharedLibrary,
    root: PathBuf,
    sets: BTreeMap<String, Vec<String>>,
}

fn read_source_sets(root: &Path) -> Result<BTreeMap<String, Vec<String>>, String> {
    let build = root.join("libobs/build");
    let manifest = build.join("source-sets.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let mut sets: BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let fragments = build.join("ported-source-sets");
    println!("cargo:rerun-if-changed={}", fragments.display());
    if fragments.exists() {
        let mut paths = fs::read_dir(&fragments)
            .map_err(|e| format!("read {}: {e}", fragments.display()))?
            .map(|entry| entry.map(|entry| entry.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        for path in paths
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        {
            println!("cargo:rerun-if-changed={}", path.display());
            let additions: BTreeMap<String, Vec<String>> =
                serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("read {}: {e}", path.display()))?;
            for (name, members) in additions {
                if !name.ends_with("_PORTED_SOURCES") {
                    return Err(format!(
                        "unknown ported source set in {}: {name}",
                        path.display()
                    ));
                }
                let Some(existing) = sets.get_mut(&name) else {
                    return Err(format!(
                        "unknown ported source set in {}: {name}",
                        path.display()
                    ));
                };
                for member in members {
                    if existing.contains(&member) {
                        return Err(format!(
                            "duplicate ported source in {}: {member}",
                            path.display()
                        ));
                    }
                    existing.push(member);
                }
            }
        }
    }
    for (name, members) in &mut sets {
        if name.ends_with("_PORTED_SOURCES") {
            members.sort();
        }
    }
    Ok(sets)
}

fn prepare(build: &NativeBuild, root: &Path, options: &Options) -> Result<Plan, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let sets = read_source_sets(&root)?;
    let config = build.output.join("config");
    fs::create_dir_all(&config).map_err(|e| e.to_string())?;
    let config = config.canonicalize().map_err(|e| e.to_string())?;
    let mut library = SharedLibrary {
        name: "obs".into(),
        include_dirs: vec![
            root.clone(),
            root.join("libobs"),
            root.join("deps/libcaption"),
            root.join("deps/libcaption/caption"),
            config.clone(),
        ],
        definitions: vec![
            ("IS_LIBOBS".into(), None),
            ("__STDC_CONSTANT_MACROS".into(), None),
        ],
        // Source manifests already contain explicit unity groups. Preserve their
        // boundaries: merging all groups into one TU would introduce collisions.
        unity: false,
        ..SharedLibrary::default()
    };
    append_sources(&sets, "LIBOBS_SOURCES", &root, &mut library)?;
    if options.hevc {
        library.definitions.push(("ENABLE_HEVC".into(), None));
    }
    crate::platform::configure_libobs(build, &root, options, &sets, &mut library)?;
    crate::platform::configure_core_dependencies(&build.target, options, &mut library)?;
    library.dependencies.push(build_caption(build, &root)?);
    let mut header = String::from(
        "#pragma once\n#define OBS_DATA_PATH \"share/obs\"\n#define OBS_PLUGIN_PATH \"lib64/obs-modules/core\"\n#define OBS_PLUGIN_DESTINATION \"lib64/obs-modules\"\n#define OBS_RELEASE_CANDIDATE 0\n#define OBS_BETA 6\n",
    );
    crate::platform::configure_core_features(&build.target, options, &mut header);
    write_if_changed(&config.join("obsconfig.h"), &header)?;
    let version = fs::read_to_string(root.join("libobs/obsversion.c.in"))
        .map_err(|e| e.to_string())?
        .replace("@OBS_VERSION@", "33.0.0-cc-build")
        .replace("@OBS_VERSION_CANONICAL@", "33.0.0")
        .replace("@OBS_COMMIT@", "cc-build");
    let version_source = config.join("obsversion.c");
    write_if_changed(&version_source, &version)?;
    library.sources.push(version_source);
    let names = options
        .core_modules
        .iter()
        .map(|name| serde_json::to_string(name).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let module_names = if names.is_empty() {
        "NULL".into()
    } else {
        names.join(",\n")
    };
    let modules = fs::read_to_string(root.join("libobs/obs-core-modules.c.in"))
        .map_err(|e| e.to_string())?
        .replace("@OBS_CORE_MODULE_LIST@", &module_names)
        .replace("@OBS_CORE_MODULE_COUNT@", &names.len().to_string());
    let module_source = config.join("obs-core-modules.c");
    write_if_changed(&module_source, &modules)?;
    library.sources.push(module_source);
    if !options.unity {
        build.separate_target_sources(&root, &mut library)?;
    }
    if options.pch {
        library.precompiled_headers = crate::groups::headers(&root, &library.sources)?;
    }
    Ok(Plan {
        common: library,
        root,
        sets,
    })
}

fn finish(build: &NativeBuild, library: &SharedLibrary) -> Result<PathBuf, String> {
    let artifact = build.shared_library(library)?;
    let linker_name =
        artifact.with_file_name(crate::platform::shared_filename(&build.target, "obs"));
    if linker_name != artifact {
        fs::copy(&artifact, &linker_name)
            .map_err(|e| format!("create {}: {e}", linker_name.display()))?;
    }
    crate::platform::stage_core_identity(&build.target, &artifact)?;
    Ok(artifact)
}

fn add_variant(
    build: &NativeBuild,
    root: &Path,
    sets: &BTreeMap<String, Vec<String>>,
    options: &Options,
    library: &mut SharedLibrary,
    rust_archive: Option<&Path>,
) -> Result<(), String> {
    if rust_archive.is_some() {
        append_sources(sets, "LIBOBS_RUST_SOURCES", root, library)?;
    } else {
        append_sources(sets, "LIBOBS_PORTED_SOURCES", root, library)?;
        if options.hevc {
            append_sources(sets, "LIBOBS_HEVC_PORTED_SOURCES", root, library)?;
        }
    }
    crate::platform::configure_libobs_variant(build, root, options, sets, library, rust_archive)?;
    if !options.unity {
        build.separate_target_sources(root, library)?;
    }
    if options.pch {
        library.precompiled_headers = crate::groups::headers(root, &library.sources)?;
    }
    Ok(())
}

pub(crate) fn append_sources(
    sets: &BTreeMap<String, Vec<String>>,
    name: &str,
    root: &Path,
    library: &mut SharedLibrary,
) -> Result<(), String> {
    let sources = sets
        .get(name)
        .ok_or_else(|| format!("missing source set {name}"))?;
    library.sources.extend(
        sources
            .iter()
            .filter(|s| {
                s.ends_with(".c") || s.ends_with(".cpp") || s.ends_with(".m") || s.ends_with(".mm")
            })
            .map(|s| root.join("libobs").join(s)),
    );
    Ok(())
}

pub(crate) fn add_package(
    library: &mut SharedLibrary,
    target: &str,
    package: &str,
) -> Result<(), String> {
    let (host, _) = crate::qt::build_triples();
    if let Some(qt) = crate::qt::package(&host, target, package, &crate::qt::Inputs::from_env())? {
        library.include_dirs.extend(qt.include_dirs);
        library.definitions.extend(qt.definitions);
        library.compile_options.extend(qt.compile_options);
        library.c_compile_options.extend(qt.c_compile_options);
        library.cpp_compile_options.extend(qt.cpp_compile_options);
        library.link_options.extend(qt.link_options);
        return Ok(());
    }
    let dependency = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe(package)
        .map_err(|e| format!("native dependency {package}: {e}"))?;
    library.include_dirs.extend(dependency.include_paths);
    // pkg-config stores these in a randomly seeded HashMap. Stable command
    // ordering prevents equivalent definitions from changing compiler cache keys.
    let mut definitions: Vec<_> = dependency.defines.into_iter().collect();
    definitions.sort();
    library.definitions.extend(definitions);
    library
        .link_options
        .extend(crate::platform::package_link_options(
            target,
            &dependency.link_paths,
            &dependency.libs,
        ));
    Ok(())
}

pub(crate) fn build_caption(build: &NativeBuild, root: &Path) -> Result<PathBuf, String> {
    let mut caption = SharedLibrary {
        name: "caption".into(),
        include_dirs: vec![
            root.join("deps/libcaption"),
            root.join("deps/libcaption/caption"),
        ],
        compile_options: vec![],
        definitions: vec![("__STDC_CONSTANT_MACROS".into(), None)],
        ..SharedLibrary::default()
    };
    crate::platform::configure_caption(&build.target, &mut caption)?;
    for source in [
        "caption",
        "cea708",
        "eia608",
        "eia608_charmap",
        "eia608_from_utf8",
        "mpeg",
        "scc",
        "srt",
        "utf8",
        "xds",
    ] {
        caption
            .sources
            .push(root.join(format!("deps/libcaption/src/{source}.c")));
    }
    build.static_library(&caption)
}

fn write_if_changed(path: &Path, text: &str) -> Result<(), String> {
    if fs::read_to_string(path).ok().as_deref() != Some(text) {
        fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Explicit ABI-validation pair; production installs the hybrid only.
#[derive(Debug)]
pub struct Libraries {
    pub reference: PathBuf,
    pub with_rust: PathBuf,
}

/// Compile common native sources once, then link C and Rust implementation twins.
pub fn build_pair(
    build: &NativeBuild,
    root: &Path,
    options: &Options,
    rust_archive: &Path,
) -> Result<Libraries, String> {
    // The Rust staticlib is built by the outer Cargo invocation, never a nested
    // Cargo process inside obs-sys/build.rs (which would wait on its own lock).
    if !rust_archive.is_file() {
        return Err(format!(
            "Rust archive does not exist: {}",
            rust_archive.display()
        ));
    }
    let plan = prepare(build, root, options)?;
    let mut common = plan.common;
    common.name = "obs-common".into();
    let (_, _, objects) = build.objects(&common)?;
    common.sources.clear();
    common.precompiled_headers.clear();
    common.source_compile_options.clear();
    common.resource_files.clear();
    common.resource_compile_options.clear();
    common.resource_input_files.clear();
    common.precompiled_objects = objects;
    let mut reference = common.clone();
    reference.name = "obs-c-reference".into();
    add_variant(build, &plan.root, &plan.sets, options, &mut reference, None)?;
    let mut with_rust = common;
    with_rust.name = "obs".into();
    add_variant(
        build,
        &plan.root,
        &plan.sets,
        options,
        &mut with_rust,
        Some(rust_archive),
    )?;
    Ok(Libraries {
        reference: finish(build, &reference)?,
        with_rust: finish(build, &with_rust)?,
    })
}
