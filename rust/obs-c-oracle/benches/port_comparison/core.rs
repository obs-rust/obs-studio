//! obs-core: the module search paths (`libobs/obs-module-paths.c.h`), the
//! module names and files (`libobs/obs-module-names.c.h`) and the module
//! locale files (`libobs/obs-module-locale.c.h`), on a small real tree in
//! the temporary directory. Both sides call the same `os_file_exists()` and
//! `os_glob()`, so the timings of the lookups are mostly the file system's;
//! the difference is the path building around them. The names, config and
//! manifest paths, the list match and the locale file names and decision
//! touch no file system.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::PathBuf;

use criterion::Criterion;
use obs_c_oracle::module_locale::{
    oracle_obs_module_locale_default_file, oracle_obs_module_locale_file,
    oracle_obs_module_locale_is_default,
};
use obs_c_oracle::module_names::{
    oracle_obs_module_names_config_path, oracle_obs_module_names_file_name,
    oracle_obs_module_names_find_core, oracle_obs_module_names_listed,
    oracle_obs_module_names_manifest_path, oracle_obs_module_names_module_name,
};
use obs_c_oracle::module_paths::{
    OracleModuleInfo2, oracle_obs_module_paths_data_dir, oracle_obs_module_paths_find,
    oracle_obs_module_paths_find_file,
};
use obs_core::ffi::module_locale::{
    obs_module_locale_default_file, obs_module_locale_file, obs_module_locale_is_default,
};
use obs_core::ffi::module_names::{
    obs_module_names_config_path, obs_module_names_file_name, obs_module_names_find_core,
    obs_module_names_listed, obs_module_names_manifest_path, obs_module_names_module_name,
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
            file(root.join(format!("core/c{i:02}.so")));
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

fn module_names(criterion: &mut Criterion) {
    let path = c"/usr/lib/x86_64-linux-gnu/obs-plugins/obs-ffmpeg.so";
    super::pair(
        criterion,
        "module_names/file_name",
        // SAFETY: a NUL-terminated path; the result points into it.
        || unsafe { obs_module_names_file_name(path.as_ptr()) } as usize,
        || unsafe { oracle_obs_module_names_file_name(path.as_ptr()) } as usize,
    );
    let (file, ext) = (c"obs-ffmpeg.so", c".so");
    super::pair(
        criterion,
        "module_names/module_name",
        // SAFETY: NUL-terminated strings; the result is freed.
        || take(unsafe { obs_module_names_module_name(file.as_ptr(), ext.as_ptr()) }),
        || take(unsafe { oracle_obs_module_names_module_name(file.as_ptr(), ext.as_ptr()) }),
    );
    let (config, name, json) = (
        c"/etc/obs-studio/plugin_config",
        c"obs-ffmpeg",
        c"settings.json",
    );
    super::pair(
        criterion,
        "module_names/config_path",
        // SAFETY: as above.
        || {
            take(unsafe {
                obs_module_names_config_path(config.as_ptr(), name.as_ptr(), json.as_ptr())
            })
        },
        || {
            take(unsafe {
                oracle_obs_module_names_config_path(config.as_ptr(), name.as_ptr(), json.as_ptr())
            })
        },
    );
    let data = c"/usr/share/obs/obs-plugins/obs-ffmpeg";
    super::pair(
        criterion,
        "module_names/manifest_path",
        // SAFETY: as above.
        || take(unsafe { obs_module_names_manifest_path(data.as_ptr()) }),
        || take(unsafe { oracle_obs_module_names_manifest_path(data.as_ptr()) }),
    );

    let names: Vec<CString> = (0..16)
        .map(|i| CString::new(format!("plugin-{i:02}")).unwrap())
        .collect();
    let mut list: Vec<*mut c_char> = names.iter().map(|n| n.as_ptr().cast_mut()).collect();
    let list = list.as_mut_ptr().cast_const();
    for (case, name) in [("last", c"plugin-15"), ("miss", c"plugin-16")] {
        super::pair(
            criterion,
            &format!("module_names/listed/{case}"),
            // SAFETY: 16 valid names and a valid name.
            || unsafe { obs_module_names_listed(list, 16, name.as_ptr()) },
            || unsafe { oracle_obs_module_names_listed(list, 16, name.as_ptr()) },
        );
    }

    let tree = Tree::new();
    let data = tree.at("data/%module%");
    for (case, binary, name) in [
        ("hit", tree.at("core/c03"), c"c03"),
        ("miss", tree.at("core/x03"), c"x03"),
    ] {
        super::pair(
            criterion,
            &format!("module_names/find_core/{case}"),
            || {
                let mut total = (0usize, 0usize);
                // SAFETY: valid strings and callback; `total` outlives the
                // call.
                let found = unsafe {
                    obs_module_names_find_core(
                        binary.as_ptr(),
                        name.as_ptr(),
                        data.as_ptr(),
                        0,
                        c".so".as_ptr(),
                        Some(count_shim),
                        (&raw mut total).cast(),
                    )
                };
                (found, total)
            },
            || {
                let mut total = (0usize, 0usize);
                // SAFETY: as above.
                let found = unsafe {
                    oracle_obs_module_names_find_core(
                        binary.as_ptr(),
                        name.as_ptr(),
                        data.as_ptr(),
                        0,
                        c".so".as_ptr(),
                        Some(count_oracle),
                        (&raw mut total).cast(),
                    )
                };
                (found, total)
            },
        );
    }
}

fn module_locale(criterion: &mut Criterion) {
    let (default_locale, locale) = (c"en-US", c"de-DE");
    super::pair(
        criterion,
        "module_locale/default_file",
        // SAFETY: a NUL-terminated locale; the result is freed.
        || take(unsafe { obs_module_locale_default_file(default_locale.as_ptr()) }),
        || take(unsafe { oracle_obs_module_locale_default_file(default_locale.as_ptr()) }),
    );
    super::pair(
        criterion,
        "module_locale/locale_file",
        // SAFETY: as above.
        || take(unsafe { obs_module_locale_file(locale.as_ptr()) }),
        || take(unsafe { oracle_obs_module_locale_file(locale.as_ptr()) }),
    );
    for (case, a) in [("same", c"EN-us"), ("other", c"de-DE")] {
        super::pair(
            criterion,
            &format!("module_locale/is_default/{case}"),
            // SAFETY: NUL-terminated locales.
            || unsafe { obs_module_locale_is_default(a.as_ptr(), default_locale.as_ptr()) },
            || unsafe { oracle_obs_module_locale_is_default(a.as_ptr(), default_locale.as_ptr()) },
        );
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    module_paths(criterion);
    module_names(criterion);
    module_locale(criterion);
}
