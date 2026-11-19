//! The same real, demand-linked bridge members selected by cmocka's CMake graph.
use super::{Dependency, NativeBuild, SharedLibrary};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MANIFEST: &str = include_str!("../../../../../test/cmocka/build/rust-static-support.json");

pub(super) fn sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        sources: Vec<String>,
    }
    let manifest: Manifest = serde_json::from_str(MANIFEST)
        .map_err(|error| format!("Rust-static support manifest: {error}"))?;
    Ok(manifest
        .sources
        .iter()
        .map(|path| root.join(path))
        .collect())
}

pub(super) struct Support {
    archive: PathBuf,
    caption: PathBuf,
}

fn prepare(
    build: &NativeBuild,
    root: &Path,
    libraries: &BTreeMap<String, Dependency>,
) -> Result<Support, String> {
    let mut support = SharedLibrary {
        name: "rust-static-link-support".into(),
        sources: sources(root)?,
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            root.join("deps/libcaption"),
        ],
        ..Default::default()
    };
    // These are owner interfaces, not a list repeated for each executable.
    for name in [
        "OBS::libobs",
        "SIMDe::SIMDe",
        "Uthash::Uthash",
        "libobs_rust-static",
    ] {
        if let Some(owner) = libraries.get(name) {
            support
                .include_dirs
                .extend(owner.include_dirs.iter().cloned());
            support
                .definitions
                .extend(owner.definitions.iter().cloned());
        }
    }
    println!(
        "cargo:rerun-if-changed={}",
        root.join("test/cmocka/build/rust-static-support.json")
            .display()
    );
    Ok(Support {
        archive: build.static_library(&support)?,
        caption: crate::libobs::build_caption(build, root)?,
    })
}

pub(super) fn select(
    build: &NativeBuild,
    root: &Path,
    libraries: &BTreeMap<String, Dependency>,
    names: &[String],
    target: &mut SharedLibrary,
    cached: &mut Option<Support>,
) -> Result<(), String> {
    if !names.iter().any(|name| {
        name == "libobs_rust-static"
            && libraries
                .get(name)
                .is_some_and(|dep| dep.rust_static_support)
    }) {
        return Ok(());
    }
    let support = match cached {
        Some(support) => support,
        None => cached.insert(prepare(build, root, libraries)?),
    };
    // Private adapters follow Rust, including when a fixture deliberately
    // selects a shared provider before that archive.
    let archive = libraries["libobs_rust-static"]
        .artifacts
        .first()
        .ok_or("Rust-static dependency has no archive")?;
    let index = target
        .dependencies
        .iter()
        .position(|path| path == archive)
        .ok_or("Rust-static archive missing from native link")?;
    target
        .dependencies
        .insert(index + 1, support.archive.clone());
    // A support member can introduce a previously unseen Rust callback.
    target.dependencies.insert(index + 2, archive.clone());
    target.include_dirs.push(root.join("deps/libcaption"));
    target
        .link_options
        .push(support.caption.to_string_lossy().into_owned());
    // Legacy registrations may still name these eager wrappers. The manifest
    // now owns their real implementations lazily.
    let shared_sources = sources(root)?;
    target.sources.retain(|path| !shared_sources.contains(path));
    Ok(())
}
