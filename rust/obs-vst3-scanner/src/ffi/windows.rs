use std::ffi::c_int;
use std::path::{Path, PathBuf};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetErrorMode(mode: u32) -> u32;
    fn SetSearchPathMode(flags: u32) -> c_int;
    fn SetDefaultDllDirectories(flags: u32) -> c_int;
    fn SetDllDirectoryW(path: *const u16) -> c_int;
}

pub(crate) fn prepare_process() {
    const SEM_FAILCRITICALERRORS: u32 = 1;
    const BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE: u32 = 1;
    const BASE_SEARCH_PATH_PERMANENT: u32 = 0x8000;
    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x1000;

    // SAFETY: These change the dedicated scanner process's loader defaults.
    // The array passed to SetDllDirectoryW is a live NUL-terminated string.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS);
        SetSearchPathMode(BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE | BASE_SEARCH_PATH_PERMANENT);
        SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
        SetDllDirectoryW([0_u16].as_ptr());
    }
}

pub(crate) fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    roots.extend(
        std::env::var_os("ProgramFiles")
            .map(|path| PathBuf::from(path).join("Common Files").join("VST3")),
    );
    roots.extend(std::env::var_os("LOCALAPPDATA").map(|path| {
        PathBuf::from(path)
            .join("Programs")
            .join("Common")
            .join("VST3")
    }));
    roots
}

pub(crate) fn is_candidate(path: &Path) -> bool {
    path.is_file()
}

pub(crate) fn metadata_root(path: &Path) -> Option<&Path> {
    path.ancestors().nth(3)
}
