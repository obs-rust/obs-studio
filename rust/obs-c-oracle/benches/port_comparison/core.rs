//! obs-core: the module search paths (`libobs/obs-module-paths.c.h`), on a
//! small real tree in the temporary directory. Both sides call the same
//! `os_file_exists()` and `os_glob()`, so the timings are mostly the file
//! system's; the difference is the path building around it.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::PathBuf;

use criterion::Criterion;
use obs_c_oracle::module_paths::{
    OracleModuleInfo2, oracle_obs_module_paths_data_dir, oracle_obs_module_paths_find,
    oracle_obs_module_paths_find_file,
};
use obs_core::ffi::module_paths::{
    obs_module_info2, obs_module_paths_data_dir, obs_module_paths_find, obs_module_paths_find_file,
};
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::dstr::{dstr, dstr_ncat};

struct Tree(PathBuf);

impl Tree {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("obs-module-paths-bench-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for i in 0..16 {
            let file = |p: PathBuf| {
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, b"").unwrap();
            };
            file(root.join(format!("plugins/m{i:02}.so")));
            file(root.join(format!("plugins/libl{i:02}.so")));
            file(root.join(format!("mods/m{i:02}/bin/64bit/m{i:02}.so")));
            std::fs::create_dir_all(root.join(format!("data/m{i:02}/locale"))).unwrap();
            file(root.join(format!("data/m{i:02}/locale/en-US.ini")));
        }
        Self(root)
    }

    fn at(&self, rel: &str) -> CString {
        CString::new(format!(
            "{}/{rel}",
            self.0.to_str().unwrap().replace('\\', "/")
        ))
        .unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn take(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated `bmalloc`'d string from a shim or the oracle.
    let out = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: allocated by `bmalloc`.
    unsafe { bfree(p.cast()) };
    Some(out)
}

unsafe extern "C" fn hook_so(d: *mut dstr) {
    // SAFETY: a valid `dstr` and a 3-byte string.
    unsafe { dstr_ncat(d, c".so".as_ptr(), 3) };
}

unsafe extern "C" fn hook_so_oracle(d: *mut c_void) {
    // SAFETY: the oracle passes its `struct dstr *`.
    unsafe { hook_so(d.cast()) };
}

/// Counts the modules and sums the lengths of their data paths.
unsafe extern "C" fn count_shim(param: *mut c_void, info: *const obs_module_info2) {
    // SAFETY: `param` is a `(usize, usize)`, `info` valid for the call.
    unsafe {
        let total = &mut *param.cast::<(usize, usize)>();
        total.0 += 1;
        total.1 += CStr::from_ptr((*info).data_path).to_bytes().len();
    }
}

unsafe extern "C" fn count_oracle(param: *mut c_void, info: *const OracleModuleInfo2) {
    // SAFETY: as `count_shim`.
    unsafe {
        let total = &mut *param.cast::<(usize, usize)>();
        total.0 += 1;
        total.1 += CStr::from_ptr((*info).data_path).to_bytes().len();
    }
}

fn module_paths(criterion: &mut Criterion) {
    let tree = Tree::new();
    let data = tree.at("data/%module%");
    let ext = c".so";

    let name = c"m03";
    super::pair(
        criterion,
        "module_paths/data_dir",
        // SAFETY: NUL-terminated strings; the result is freed.
        || take(unsafe { obs_module_paths_data_dir(name.as_ptr(), data.as_ptr()) }),
        || take(unsafe { oracle_obs_module_paths_data_dir(name.as_ptr(), data.as_ptr()) }),
    );
    let lib = c"libm03";
    super::pair(
        criterion,
        "module_paths/data_dir_lib",
        // SAFETY: as above.
        || take(unsafe { obs_module_paths_data_dir(lib.as_ptr(), data.as_ptr()) }),
        || take(unsafe { oracle_obs_module_paths_data_dir(lib.as_ptr(), data.as_ptr()) }),
    );

    let stub = tree.at("data/m03");
    for (case, file) in [("hit", c"locale/en-US.ini"), ("miss", c"locale/xx-XX.ini")] {
        super::pair(
            criterion,
            &format!("module_paths/find_file/{case}"),
            // SAFETY: as above.
            || take(unsafe { obs_module_paths_find_file(stub.as_ptr(), file.as_ptr()) }),
            || take(unsafe { oracle_obs_module_paths_find_file(stub.as_ptr(), file.as_ptr()) }),
        );
    }

    for (case, bin) in [
        ("flat", tree.at("plugins")),
        ("module_dirs", tree.at("mods/%module%/bin/64bit")),
    ] {
        super::pair(
            criterion,
            &format!("module_paths/find/{case}"),
            || {
                let mut total = (0usize, 0usize);
                // SAFETY: valid strings, hook and callback; `total` outlives
                // the call.
                unsafe {
                    obs_module_paths_find(
                        bin.as_ptr(),
                        data.as_ptr(),
                        2 as c_int,
                        ext.as_ptr(),
                        Some(hook_so),
                        Some(count_shim),
                        (&raw mut total).cast(),
                    );
                }
                total
            },
            || {
                let mut total = (0usize, 0usize);
                // SAFETY: as above.
                unsafe {
                    oracle_obs_module_paths_find(
                        bin.as_ptr(),
                        data.as_ptr(),
                        2 as c_int,
                        ext.as_ptr(),
                        Some(hook_so_oracle),
                        Some(count_oracle),
                        (&raw mut total).cast(),
                    );
                }
                total
            },
        );
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    module_paths(criterion);
}
