use crate::cache::Class;
use crate::ffi::{self, GetFactory};
use libloading::os::unix::Library;
use std::ffi::c_void;
use std::path::{Path, PathBuf};

pub(crate) const WINDOWS_COM_ORDER: bool = false;

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

pub(crate) fn enumerate(bundle: &Path) -> Vec<Class> {
    let Some(stem) = bundle.file_stem() else {
        return Vec::new();
    };
    let mut binary = stem.to_owned();
    binary.push(".so");
    let path = bundle
        .join("Contents")
        .join(format!("{}-linux", std::env::consts::ARCH))
        .join(binary);

    // SAFETY: The scanner process deliberately loads installed VST3 binaries.
    // These signatures come from the pinned SDK's module_linux.cpp, and all
    // factory interfaces are released before ModuleExit and dlclose.
    unsafe {
        let Ok(library) = Library::new(path) else {
            return Vec::new();
        };
        let handle = library.into_raw();
        let library = Library::from_raw(handle);
        let (Ok(entry), Ok(exit), Ok(factory)) = (
            library.get::<unsafe extern "system" fn(*mut c_void) -> bool>(b"ModuleEntry\0"),
            library.get::<unsafe extern "system" fn() -> bool>(b"ModuleExit\0"),
            library.get::<GetFactory>(b"GetPluginFactory\0"),
        ) else {
            return Vec::new();
        };
        if !entry(handle) {
            return Vec::new();
        }
        let classes = ffi::classes(factory(), bundle);
        exit();
        classes
    }
}
