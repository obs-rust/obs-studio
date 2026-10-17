//! The native libobs graph, shared by bundled obs-sys and the application build.
use crate::{NativeBuild, SharedLibrary};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Options {
    pub hevc: bool,
    pub wayland: bool,
    pub pulseaudio: bool,
    pub gio: bool,
    pub core_modules: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            hevc: true,
            wayland: true,
            pulseaudio: true,
            gio: true,
            core_modules: vec![],
        }
    }
}

/// Build the real C libobs, including generated configuration and native dependencies.
pub fn build(build: &NativeBuild, root: &Path) -> Result<PathBuf, String> {
    build_with_options(build, root, &Options::default())
}

pub fn build_with_options(
    build: &NativeBuild,
    root: &Path,
    options: &Options,
) -> Result<PathBuf, String> {
    let plan = prepare(build, root, options)?;
    let mut library = plan.common;
    add_variant(build, &plan.root, &plan.sets, options, &mut library, None)?;
    finish(build, &library)
}

struct Plan {
    common: SharedLibrary,
    root: PathBuf,
    sets: BTreeMap<String, Vec<String>>,
}

fn prepare(build: &NativeBuild, root: &Path, options: &Options) -> Result<Plan, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let manifest = root.join("libobs/build/source-sets.json");
    let sets: BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| format!("read {}: {e}", manifest.display()))?;
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
    crate::platform::configure_libobs(&build.target, &root, options, &sets, &mut library)?;
    for package in [
        "libavformat",
        "libavcodec",
        "libavutil",
        "libswscale",
        "libswresample",
        "jansson",
        "zlib",
        "simde",
    ] {
        add_package(&mut library, &build.target, package)?;
    }
    library.dependencies.push(build_caption(build, &root)?);
    let mut header = String::from(
        "#pragma once\n#define OBS_DATA_PATH \"share/obs\"\n#define OBS_PLUGIN_PATH \"lib64/obs-modules/core\"\n#define OBS_PLUGIN_DESTINATION \"lib64/obs-modules\"\n#define OBS_RELEASE_CANDIDATE 0\n#define OBS_BETA 6\n",
    );
    for (enabled, name) in [
        (options.gio, "GIO_FOUND"),
        (options.pulseaudio, "PULSEAUDIO_FOUND"),
        (options.wayland, "ENABLE_WAYLAND"),
    ] {
        if enabled {
            header.push_str(&format!("#define {name}\n"));
        }
    }
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
            library
                .sources
                .push(root.join("libobs/build/libobs.ported-hevc+.h.c"));
        }
    }
    crate::platform::configure_libobs_variant(build, root, options, sets, library, rust_archive)
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
    for path in &dependency.link_paths {
        library.link_options.push(format!("-L{}", path.display()));
        library
            .link_options
            .push(format!("-Wl,-rpath,{}", path.display()));
    }
    for name in dependency.libs {
        // Some pkg-config files list a static archive directory before their
        // shared directory. Resolve across every directory before allowing the
        // linker to fall back to -l, which otherwise silently embeds the archive.
        if let Some(path) =
            crate::platform::package_shared_library(target, &dependency.link_paths, &name)
        {
            library.link_options.push(path.display().to_string());
        } else {
            library.link_options.push(format!("-l{name}"));
        }
    }
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

/// Both ABI-compatible variants; only the C reference is installed.
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
    common.precompiled_objects = objects;
    let mut reference = common.clone();
    reference.name = "obs".into();
    add_variant(build, &plan.root, &plan.sets, options, &mut reference, None)?;
    let mut with_rust = common;
    with_rust.name = "obs-with-rust".into();
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
