//! Safe core for `os_mkdirs` from `libobs/util/platform-mkdirs.c`
//! (obs-rust/obs-studio#262 slice 4): create a directory and every missing
//! parent, over the host's `os_mkdir` in the [platform boundary](crate::platform).

/// `MKDIR_SUCCESS`: the directory was created.
pub const MKDIR_SUCCESS: i32 = 0;
/// `MKDIR_EXISTS`: something already exists at the path.
pub const MKDIR_EXISTS: i32 = 1;
/// `MKDIR_ERROR`: any other failure.
pub const MKDIR_ERROR: i32 = -1;

/// `os_mkdirs(dir)`: `\` becomes `/`, then the path is created the way
/// `recursive_mkdir` does it. The result is that of the last `os_mkdir`, so
/// `"a/b/"` with `a` missing creates both and reports [`MKDIR_EXISTS`].
pub fn mkdirs(dir: &[u8]) -> i32 {
    let _ = dir;
    todo!("os_mkdirs port")
}
