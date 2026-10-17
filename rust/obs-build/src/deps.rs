//! Shared native archives used by plugins and the frontend.
use crate::{NativeBuild, SharedLibrary, libobs::add_package};
use std::path::{Path, PathBuf};

pub(crate) fn shared_archive(
    build: &NativeBuild,
    root: &Path,
    name: &str,
) -> Result<PathBuf, String> {
    let mut library = SharedLibrary {
        name: name.into(),
        sources: vec![root.join(format!("shared/{name}/build/{name}.core+.h.c"))],
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            root.join("shared").join(name),
            root.join("deps/libcaption"),
            root.join("deps/libcaption/caption"),
            build.output.join("config"),
        ],
        ..SharedLibrary::default()
    };
    crate::platform::configure_shared_dependency(&build.target, root, name, &mut library)?;
    add_package(&mut library, &build.target, "simde")?;
    build.static_library(&library)
}
