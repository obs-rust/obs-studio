use std::ffi::{CStr, c_void};
use std::hint::black_box;
use std::ptr;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use obs_c_oracle::{bmem as c_mem, crc32 as c_crc, dstr as c_str, dstr_libc as c_wide};
use obs_util::ffi::utf8::WChar;
use obs_util::ffi::{bmem as rs_mem, crc32 as rs_crc, dstr as rs_str, dstr_libc as rs_wide};

type Allocate = unsafe extern "C" fn(usize) -> *mut c_void;
type Release = unsafe extern "C" fn(*mut c_void);
type Resize = unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void;
type Crc = unsafe extern "C" fn(u32, *const c_void, usize) -> u32;
type Duplicate = unsafe extern "C" fn(*const c_void, usize) -> *mut c_void;

unsafe extern "C" {
    fn oracle_dstr_printf(
        destination: *mut c_str::OracleDstr,
        format: *const std::ffi::c_char,
        ...
    );
}

struct Allocation {
    pointer: *mut c_void,
    release: Release,
}

impl Allocation {
    fn new(allocate: Allocate, release: Release, size: usize) -> Self {
        // SAFETY: the selected allocator accepts any requested size.
        let pointer = unsafe { allocate(size) };
        assert!(!pointer.is_null());
        assert_eq!(pointer as usize % 32, 0);
        Self { pointer, release }
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        // SAFETY: this owns the pointer, freed once by its matching allocator.
        unsafe { (self.release)(self.pointer) };
    }
}

fn memory(criterion: &mut Criterion) {
    for size in [64, 4096, 65536] {
        let mut group = criterion.benchmark_group(format!("bmem/alloc_free/{size}"));
        for (name, allocate, release) in [
            (
                "rust",
                rs_mem::bmalloc as Allocate,
                rs_mem::bfree as Release,
            ),
            (
                "c",
                c_mem::oracle_bmalloc as Allocate,
                c_mem::oracle_bfree as Release,
            ),
        ] {
            group.bench_function(name, |b| {
                b.iter(|| {
                    // SAFETY: allocation and free use the same implementation;
                    // the pointer is observed but uninitialized bytes aren't read.
                    unsafe {
                        let pointer = allocate(black_box(size));
                        black_box(pointer);
                        release(pointer);
                    }
                });
            });
        }
        group.finish();

        let mut allocate_group = criterion.benchmark_group(format!("bmem/allocate/{size}"));
        for (name, allocate, release) in [
            (
                "rust",
                rs_mem::bmalloc as Allocate,
                rs_mem::bfree as Release,
            ),
            (
                "c",
                c_mem::oracle_bmalloc as Allocate,
                c_mem::oracle_bfree as Release,
            ),
        ] {
            drop(Allocation::new(allocate, release, size));
            allocate_group.bench_function(name, |b| {
                b.iter_batched(
                    || (),
                    |()| {
                        // SAFETY: positive allocation size; the returned ownership
                        // wrapper frees it outside the timed batch, without reading it.
                        let pointer = unsafe { allocate(black_box(size)) };
                        black_box(Allocation { pointer, release })
                    },
                    BatchSize::NumIterations(64),
                )
            });
        }
        allocate_group.finish();

        let mut free_group = criterion.benchmark_group(format!("bmem/free/{size}"));
        for (name, allocate, release) in [
            (
                "rust",
                rs_mem::bmalloc as Allocate,
                rs_mem::bfree as Release,
            ),
            (
                "c",
                c_mem::oracle_bmalloc as Allocate,
                c_mem::oracle_bfree as Release,
            ),
        ] {
            free_group.bench_function(name, |b| {
                b.iter_batched(
                    || Allocation::new(allocate, release, size),
                    drop,
                    BatchSize::NumIterations(64),
                )
            });
        }
        free_group.finish();

        let mut group = criterion.benchmark_group(format!("bmem/realloc/{size}"));
        for (name, allocate, resize, release) in [
            (
                "rust",
                rs_mem::bmalloc as Allocate,
                rs_mem::brealloc as Resize,
                rs_mem::bfree as Release,
            ),
            (
                "c",
                c_mem::oracle_bmalloc as Allocate,
                c_mem::oracle_brealloc as Resize,
                c_mem::oracle_bfree as Release,
            ),
        ] {
            group.bench_function(name, |b| {
                b.iter_batched(
                    || Allocation::new(allocate, release, size),
                    |mut allocation| {
                        // SAFETY: resize receives its own live allocation.
                        allocation.pointer =
                            unsafe { resize(allocation.pointer, black_box(size * 2)) };
                        black_box(allocation)
                    },
                    BatchSize::NumIterations(64),
                );
            });
        }
        group.finish();

        let input = vec![0x55u8; size];
        let mut group = criterion.benchmark_group(format!("bmem/dup_free/{size}"));
        for (name, duplicate, release) in [
            (
                "rust",
                rs_mem::bmemdup as Duplicate,
                rs_mem::bfree as Release,
            ),
            (
                "c",
                c_mem::oracle_bmemdup as Duplicate,
                c_mem::oracle_bfree as Release,
            ),
        ] {
            // SAFETY: input is readable for size bytes and the duplicate
            // is owned here, checked before release by its matching allocator.
            unsafe {
                let pointer = duplicate(input.as_ptr().cast(), size);
                assert_eq!(
                    std::slice::from_raw_parts(pointer.cast::<u8>(), size),
                    input
                );
                release(pointer);
            }
            group.bench_function(name, |b| {
                b.iter(|| {
                    // SAFETY: input remains readable; duplicate is freed once
                    // by the matching implementation, with no uninitialized read.
                    unsafe {
                        let pointer = duplicate(black_box(input.as_ptr().cast()), size);
                        black_box(pointer);
                        release(pointer);
                    }
                })
            });
        }
        group.finish();
    }
    super::pair(
        criterion,
        "bmem/count",
        || {
            // SAFETY: reads only the implementation's allocation counter.
            unsafe { rs_mem::bnum_allocs() }
        },
        || {
            // SAFETY: reads only the independent C allocator's counter.
            unsafe { c_mem::oracle_bnum_allocs() }
        },
    );
    super::pair(
        criterion,
        "bmem/alignment",
        || {
            // SAFETY: reads the allocator's public alignment constant.
            unsafe { rs_mem::base_get_alignment() }
        },
        || {
            // SAFETY: reads the original allocator's alignment constant.
            unsafe { c_mem::oracle_base_get_alignment() }
        },
    );
}

struct RustString(rs_str::dstr);
struct Cstring(c_str::OracleDstr);

impl RustString {
    fn new(seed: &CStr) -> Self {
        let mut string = Self(rs_str::dstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        });
        // SAFETY: initialized empty destination and NUL-terminated seed.
        unsafe { rs_str::dstr_copy(&mut string.0, seed.as_ptr()) };
        string
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: dstr holds a NUL-terminated string after each operation.
        unsafe { CStr::from_ptr(self.0.array) }.to_bytes()
    }
}

impl Drop for RustString {
    fn drop(&mut self) {
        // SAFETY: this owns its bmalloc buffer, freed once.
        unsafe { rs_mem::bfree(self.0.array.cast()) };
    }
}

impl Cstring {
    fn new(seed: &CStr) -> Self {
        let mut string = Self(c_str::OracleDstr {
            array: ptr::null_mut(),
            len: 0,
            capacity: 0,
        });
        // SAFETY: initialized empty destination and NUL-terminated seed.
        unsafe { c_str::oracle_dstr_copy(&mut string.0, seed.as_ptr()) };
        string
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: dstr holds a NUL-terminated string after each operation.
        unsafe { CStr::from_ptr(self.0.array) }.to_bytes()
    }
}

impl Drop for Cstring {
    fn drop(&mut self) {
        // SAFETY: the dstr oracle uses the shared libobs allocator, not oracle_bmalloc.
        unsafe { rs_mem::bfree(self.0.array.cast()) };
    }
}

fn strings(criterion: &mut Criterion) {
    for size in [128, 4096] {
        let seed = std::ffi::CString::new("obs ".repeat(size / 4)).unwrap();
        for operation in ["cat", "safe_printf", "replace"] {
            let rust = |string: &mut RustString| {
                // SAFETY: the destination owns a valid dstr; all input strings
                // have static NUL-terminated storage valid throughout the call.
                unsafe {
                    match operation {
                        "cat" => {
                            rs_str::dstr_ncat(&mut string.0, black_box(c" capture".as_ptr()), 8)
                        }
                        "safe_printf" => rs_str::dstr_safe_printf(
                            &mut string.0,
                            c"$1: $2".as_ptr(),
                            black_box(seed.as_ptr()),
                            c"42".as_ptr(),
                            ptr::null(),
                            ptr::null(),
                        ),
                        "replace" => rs_str::dstr_replace(
                            &mut string.0,
                            black_box(c"obs".as_ptr()),
                            c"Rust OBS".as_ptr(),
                        ),
                        _ => unreachable!(),
                    }
                }
            };
            let original = |string: &mut Cstring| {
                // SAFETY: same destination and string contracts as the Rust call.
                unsafe {
                    match operation {
                        "cat" => c_str::oracle_dstr_ncat(
                            &mut string.0,
                            black_box(c" capture".as_ptr()),
                            8,
                        ),
                        "safe_printf" => c_str::oracle_dstr_safe_printf(
                            &mut string.0,
                            c"$1: $2".as_ptr(),
                            black_box(seed.as_ptr()),
                            c"42".as_ptr(),
                            ptr::null(),
                            ptr::null(),
                        ),
                        "replace" => c_str::oracle_dstr_replace(
                            &mut string.0,
                            black_box(c"obs".as_ptr()),
                            c"Rust OBS".as_ptr(),
                        ),
                        _ => unreachable!(),
                    }
                }
            };
            let mut r = RustString::new(&seed);
            let mut c = Cstring::new(&seed);
            rust(&mut r);
            original(&mut c);
            assert_eq!(r.bytes(), c.bytes(), "dstr/{operation}/{size}");
            let mut group = criterion.benchmark_group(format!("dstr/{operation}/{size}"));
            group.bench_function("rust", |b| {
                b.iter_batched(
                    || RustString::new(&seed),
                    |mut s| {
                        rust(&mut s);
                        black_box(s)
                    },
                    BatchSize::NumIterations(64),
                )
            });
            group.bench_function("c", |b| {
                b.iter_batched(
                    || Cstring::new(&seed),
                    |mut s| {
                        original(&mut s);
                        black_box(s)
                    },
                    BatchSize::NumIterations(64),
                )
            });
            group.finish();
        }
        // Variadic printf stays C in both OFF and ON, unlike safe_printf.
        // Measure it as an explicitly named control, never a Rust port result.
        let mut r = RustString::new(&seed);
        let mut c = Cstring::new(&seed);
        // SAFETY: both destinations have the public dstr layout; the varargs
        // match %s and %d, and input strings remain NUL-terminated and live.
        unsafe {
            oracle_dstr_printf(
                (&mut r.0 as *mut rs_str::dstr).cast(),
                c"%s: %d".as_ptr(),
                seed.as_ptr(),
                42 as std::ffi::c_int,
            );
            oracle_dstr_printf(
                &mut c.0,
                c"%s: %d".as_ptr(),
                seed.as_ptr(),
                42 as std::ffi::c_int,
            );
        }
        assert_eq!(r.bytes(), c.bytes());
        let mut group = criterion.benchmark_group(format!("dstr/printf_control/{size}"));
        group.bench_function("rust", |b| {
            b.iter_batched(
                || RustString::new(&seed),
                |mut string| {
                    // SAFETY: dstr layouts are ABI-identical (existing layout tests).
                    // The format accepts a readable NUL-terminated string and c_int.
                    unsafe {
                        oracle_dstr_printf(
                            (&mut string.0 as *mut rs_str::dstr).cast(),
                            c"%s: %d".as_ptr(),
                            black_box(seed.as_ptr()),
                            42 as std::ffi::c_int,
                        )
                    };
                    black_box(string)
                },
                BatchSize::NumIterations(64),
            )
        });
        group.bench_function("c", |b| {
            b.iter_batched(
                || Cstring::new(&seed),
                |mut string| {
                    // SAFETY: initialized writable dstr and correctly typed varargs.
                    unsafe {
                        oracle_dstr_printf(
                            &mut string.0,
                            c"%s: %d".as_ptr(),
                            black_box(seed.as_ptr()),
                            42 as std::ffi::c_int,
                        )
                    };
                    black_box(string)
                },
                BatchSize::NumIterations(64),
            )
        });
        group.finish();
    }
}

/// The dstr-libc.c ports: the case map and the wide conversion
/// rebuild a dstr; the wide compare and search are pure scans. The text is
/// ASCII with one U+00E9 per 8 units, in the process's "C" locale.
fn wide_strings(criterion: &mut Criterion) {
    for size in [128, 4096] {
        let text = "Obs \u{e9}ab ".repeat(size / 8);
        let seed = std::ffi::CString::new(text.clone()).unwrap();
        let wide: Vec<WChar> = text.chars().map(|c| c as WChar).chain([0]).collect();
        let upper: Vec<WChar> = text
            .to_ascii_uppercase()
            .chars()
            .map(|c| c as WChar)
            .chain([0])
            .collect();
        let mut needle: Vec<WChar> = "NEEDLE".chars().map(|c| c as WChar).collect();
        let mut haystack = wide.clone();
        haystack.pop();
        haystack.extend("needle".chars().map(|c| c as WChar));
        haystack.push(0);
        needle.push(0);

        // Rebuilding operations, from a fresh copy of the seed each time.
        for operation in ["to_upper", "from_wcs"] {
            let rust = |string: &mut RustString| {
                // SAFETY: the destination owns a valid dstr; `wide` is a live
                // NUL-terminated wide string.
                unsafe {
                    match operation {
                        "to_upper" => rs_wide::dstr_to_upper(&mut string.0),
                        "from_wcs" => {
                            rs_wide::dstr_from_wcs(&mut string.0, black_box(wide.as_ptr()))
                        }
                        _ => unreachable!(),
                    }
                }
            };
            let original = |string: &mut Cstring| {
                // SAFETY: same destination and string contracts as the Rust call.
                unsafe {
                    match operation {
                        "to_upper" => c_wide::oracle_dstr_to_upper(&mut string.0),
                        "from_wcs" => {
                            c_wide::oracle_dstr_from_wcs(&mut string.0, black_box(wide.as_ptr()))
                        }
                        _ => unreachable!(),
                    }
                }
            };
            let mut r = RustString::new(&seed);
            let mut c = Cstring::new(&seed);
            rust(&mut r);
            original(&mut c);
            assert_eq!(r.bytes(), c.bytes(), "dstr/{operation}/{size}");
            assert_eq!(
                (r.0.len, r.0.capacity),
                (c.0.len, c.0.capacity),
                "dstr/{operation}/{size}"
            );
            let mut group = criterion.benchmark_group(format!("dstr/{operation}/{size}"));
            group.bench_function("rust", |b| {
                b.iter_batched(
                    || RustString::new(&seed),
                    |mut s| {
                        rust(&mut s);
                        black_box(s)
                    },
                    BatchSize::NumIterations(64),
                )
            });
            group.bench_function("c", |b| {
                b.iter_batched(
                    || Cstring::new(&seed),
                    |mut s| {
                        original(&mut s);
                        black_box(s)
                    },
                    BatchSize::NumIterations(64),
                )
            });
            group.finish();
        }

        // A case-insensitive compare of equal strings (it reads both to the
        // end), and a search whose only match is at the end.
        let offset = |found: *mut WChar| found as usize - haystack.as_ptr() as usize;
        super::pair(
            criterion,
            &format!("dstr/wstrcmpi/{size}"),
            // SAFETY: both are live NUL-terminated wide strings.
            || unsafe { rs_wide::wstrcmpi(black_box(wide.as_ptr()), black_box(upper.as_ptr())) },
            // SAFETY: as above.
            || unsafe {
                c_wide::oracle_wstrcmpi(black_box(wide.as_ptr()), black_box(upper.as_ptr()))
            },
        );
        super::pair(
            criterion,
            &format!("dstr/wstrstri/{size}"),
            // SAFETY: both are live NUL-terminated wide strings.
            || {
                offset(unsafe {
                    rs_wide::wstrstri(black_box(haystack.as_ptr()), black_box(needle.as_ptr()))
                })
            },
            // SAFETY: as above.
            || {
                offset(unsafe {
                    c_wide::oracle_wstrstri(
                        black_box(haystack.as_ptr()),
                        black_box(needle.as_ptr()),
                    )
                })
            },
        );
    }
}

fn crc(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("crc32");
    for size in [64, 4096, 65536] {
        let bytes: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        // SAFETY: both calls borrow the initialized input for exactly its length.
        unsafe {
            assert_eq!(
                rs_crc::calc_crc32(0, bytes.as_ptr().cast(), size),
                c_crc::oracle_calc_crc32(0, bytes.as_ptr().cast(), size)
            );
        }
        group.throughput(Throughput::Bytes(size as u64));
        for (name, function) in [
            ("rust", rs_crc::calc_crc32 as Crc),
            ("c", c_crc::oracle_calc_crc32 as Crc),
        ] {
            group.bench_with_input(BenchmarkId::new(name, size), &bytes, |b, bytes| {
                b.iter(|| {
                    // SAFETY: the black-boxed buffer is readable for its full length.
                    unsafe { function(0, black_box(bytes.as_ptr().cast()), bytes.len()) }
                })
            });
        }
    }
    group.finish();
}

pub fn benchmarks(criterion: &mut Criterion) {
    memory(criterion);
    strings(criterion);
    wide_strings(criterion);
    crc(criterion);
}
