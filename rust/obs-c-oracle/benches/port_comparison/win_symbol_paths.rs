//! obs-core: the Windows debug symbol search path rules
//! (`libobs/obs-windows-symbol-paths.c.h`), which `obs-windows.c` runs after
//! every module load pass. One measurement builds 64 search paths, each from
//! the working directory and 24 module paths as `reset_win32_symbol_paths()`
//! does (the directory of each, the duplicate check against the list, the
//! join), with each directory standing in for its absolute path, and folds
//! the search paths into a checksum that both sides must match.
//!
//! Each side fills its own `struct dstr`s (the shim through obs-util's dstr,
//! the C through the C dstr oracle) and frees them before the measurement
//! ends, and the fixtures are freed with the module, so it leaves no
//! allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! shared machine at a load average of about 21; criterion's median per 64
//! search paths): 546 us Rust, 384 us C. Most of the time is the copies, the
//! case-folded compares against the growing list and the dstr growth. The
//! shim reads each listed entry's length before comparing it, where the C's
//! `astrcmpi()` stops at the first byte that differs; it runs once per module
//! load pass, so this is left as it is.

use std::ffi::{CString, c_char};
use std::hint::black_box;
use std::ptr;

use criterion::{Criterion, Throughput};
use obs_c_oracle::dstr::OracleDstr;
use obs_c_oracle::win_symbol_paths as c;
use obs_core::ffi::win_symbol_paths as rs;
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::dstr::dstr;

const SEARCH_PATHS: usize = 64;
const MODULES: usize = 24;

/// A small deterministic generator, so both sides see the same calls.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

fn fixtures() -> Vec<Vec<CString>> {
    let mut rng = Lcg(3475);
    let mut pick = |n: u64| usize::try_from(rng.next() % n).unwrap();
    let roots = [
        "C:\\Program Files\\obs-studio",
        "C:/Program Files/obs-studio",
        "c:\\PROGRAM FILES\\OBS-Studio",
        "D:\\obs",
    ];
    let dirs = ["obs-plugins\\64bit", "obs-plugins/64bit", "bin\\64bit", "plugins\\x"];
    (0..SEARCH_PATHS)
        .map(|_| {
            (0..MODULES)
                .map(|m| {
                    let path = format!("{}\\{}\\plugin-{}.dll", roots[pick(4)], dirs[pick(4)], m % 7);
                    CString::new(path).unwrap()
                })
                .collect()
        })
        .collect()
}

struct Side {
    /// The directory, copied out of a fresh dstr that is then freed.
    dir: fn(*const c_char) -> Option<CString>,
    listed: fn(&[*mut c_char], *const c_char) -> u8,
    /// The joined bytes, copied out of a fresh dstr that is then freed.
    join: fn(&[*mut c_char]) -> Vec<u8>,
}

fn take(array: *mut c_char, len: usize, cut: bool) -> Vec<u8> {
    if array.is_null() {
        return Vec::new();
    }
    // SAFETY: a non-null dstr array holds `len` bytes.
    let mut bytes = unsafe { std::slice::from_raw_parts(array.cast::<u8>(), len) }.to_vec();
    if cut {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
        bytes.truncate(end);
    }
    // SAFETY: the array was bmalloc'd by a dstr call.
    unsafe { bfree(array.cast()) };
    bytes
}

const RUST: Side = Side {
    dir: |p| {
        let mut d = dstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        // SAFETY: `p` is NUL-terminated and `d` is a valid dstr.
        let ok = unsafe { rs::obs_windows_symbol_paths_dir(p, &raw mut d) };
        let bytes = take(d.array, d.len, true);
        ok.then(|| CString::new(bytes).unwrap())
    },
    // SAFETY: the entries are NUL-terminated.
    listed: |l, a| u8::from(unsafe { rs::obs_windows_symbol_paths_listed(l.as_ptr(), l.len(), a) }),
    join: |l| {
        let mut d = dstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        // SAFETY: the entries are NUL-terminated and `d` is a valid dstr.
        unsafe { rs::obs_windows_symbol_paths_join(l.as_ptr(), l.len(), &raw mut d) };
        take(d.array, d.len, false)
    },
};

const C: Side = Side {
    dir: |p| {
        let mut d = OracleDstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        // SAFETY: `p` is NUL-terminated and `d` is a valid dstr.
        let ok = unsafe { c::oracle_obs_windows_symbol_paths_dir(p, &raw mut d) } != 0;
        let bytes = take(d.array, d.len, true);
        ok.then(|| CString::new(bytes).unwrap())
    },
    // SAFETY: the entries are NUL-terminated.
    listed: |l, a| unsafe { c::oracle_obs_windows_symbol_paths_listed(l.as_ptr(), l.len(), a) },
    join: |l| {
        let mut d = OracleDstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        // SAFETY: the entries are NUL-terminated and `d` is a valid dstr.
        unsafe { c::oracle_obs_windows_symbol_paths_join(l.as_ptr(), l.len(), &raw mut d) };
        take(d.array, d.len, false)
    },
};

fn search_paths(side: &Side, sets: &[Vec<CString>]) -> u64 {
    let cwd = CString::new("C:\\Program Files\\obs-studio\\bin\\64bit").unwrap();
    let mut sum = 0u64;
    for modules in sets {
        let modules = black_box(modules);
        let mut owned: Vec<CString> = vec![cwd.clone()];
        let mut list: Vec<*mut c_char> = vec![owned[0].as_ptr().cast_mut()];
        for module in modules {
            let Some(dir) = (side.dir)(module.as_ptr()) else {
                continue;
            };
            if (side.listed)(&list, dir.as_ptr()) == 0 {
                list.push(dir.as_ptr().cast_mut());
                owned.push(dir);
            }
        }
        let joined = (side.join)(&list);
        sum = joined
            .iter()
            .fold(sum.wrapping_mul(31).wrapping_add(joined.len() as u64), |s, &b| {
                s.wrapping_mul(31).wrapping_add(u64::from(b))
            });
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let fixtures = fixtures();
    assert_eq!(
        search_paths(&RUST, &fixtures),
        search_paths(&C, &fixtures),
        "benchmark fixture differs: search paths"
    );
    let mut group = criterion.benchmark_group("win_symbol_paths/search_paths");
    group.throughput(Throughput::Elements(SEARCH_PATHS as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(search_paths(&RUST, &fixtures))));
    group.bench_function("c", |b| b.iter(|| black_box(search_paths(&C, &fixtures))));
    group.finish();
}
