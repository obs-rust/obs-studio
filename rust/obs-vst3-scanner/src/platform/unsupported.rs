use crate::cache::Class;
use std::path::{Path, PathBuf};

pub(crate) const WINDOWS_COM_ORDER: bool = false;
pub(crate) fn prepare_process() {}
pub(crate) fn search_roots() -> Vec<PathBuf> {
    Vec::new()
}
pub(crate) fn is_candidate(path: &Path) -> bool {
    path.is_dir()
}
pub(crate) fn metadata_root(path: &Path) -> Option<&Path> {
    Some(path)
}
pub(crate) fn enumerate(_: &Path) -> Vec<Class> {
    Vec::new()
}
