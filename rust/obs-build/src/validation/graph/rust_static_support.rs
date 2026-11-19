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

pub(super) fn prepare(
    build: &NativeBuild,
    root: &Path,
    libraries: &BTreeMap<String, Dependency>,
) -> Result<PathBuf, String> {
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
    build.static_library(&support)
}
