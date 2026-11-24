use std::path::{Path, PathBuf};

pub(crate) fn prepare_process() {}

pub(crate) fn search_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Library/Audio/Plug-Ins/VST3")];
    roots.extend(
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3")),
    );
    roots
}

pub(crate) fn is_candidate(path: &Path) -> bool {
    path.is_dir()
}

pub(crate) fn metadata_root(path: &Path) -> Option<&Path> {
    Some(path)
}
