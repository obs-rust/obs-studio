//! `os_mkdirs` from `libobs/util/platform-mkdirs.c`, compiled unmodified as
//! `oracle_os_mkdirs` by `oracle/platform.c`, over the oracle's own
//! `oracle_os_mkdir`.

use core::ffi::{c_char, c_int};

unsafe extern "C" {
    pub fn oracle_os_mkdirs(dir: *const c_char) -> c_int;
}
