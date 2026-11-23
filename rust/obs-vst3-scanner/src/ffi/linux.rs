use std::path::{Path, PathBuf};

pub(crate) fn prepare_process() {}

pub(crate) fn search_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/lib/vst3"),
        PathBuf::from("/usr/local/lib/vst3"),
    ];
    roots.extend(std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".vst3")));
    roots
}

pub(crate) fn is_candidate(path: &Path) -> bool {
    path.is_dir()
}

pub(crate) fn metadata_root(path: &Path) -> Option<&Path> {
    Some(path)
}
