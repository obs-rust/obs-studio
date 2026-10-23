use std::ffi::{CStr, c_void};
use std::hint::black_box;
use std::ptr;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use obs_c_oracle::{bmem as c_mem, crc32 as c_crc, dstr as c_str, dstr_libc as c_str_libc};
use obs_util::ffi::{bmem as rs_mem, crc32 as rs_crc, dstr as rs_str, dstr_libc as rs_str_libc};

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
        for operation in ["cat", "safe_printf", "replace", "to_upper"] {
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
                        "to_upper" => rs_str_libc::dstr_to_upper(black_box(&mut string.0)),
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
                        "to_upper" => c_str_libc::oracle_dstr_to_upper(black_box(&mut string.0)),
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

/// One profiler snapshot in the C layout, `roots` roots each with `children`
/// children, every entry with `times` time entries and as many between-call
/// entries. Both sides read the same `#[repr(C)]` layout; the bench casts
/// the pointer for the oracle. Leaked: the bench process exits after.
fn profiler_snapshot(
    roots: usize,
    children: usize,
    times: usize,
) -> *mut obs_util::ffi::profiler_snapshot::profiler_snapshot {
    use obs_util::ffi::profiler_snapshot::{
        Darray, profiler_snapshot, profiler_snapshot_entry, profiler_time_entry,
    };
    fn darray<T>(items: Vec<T>) -> Darray<T> {
        let mut items = std::mem::ManuallyDrop::new(items);
        Darray {
            array: items.as_mut_ptr(),
            num: items.len(),
            capacity: items.capacity(),
        }
    }
    fn entry(kids: Vec<profiler_snapshot_entry>, times: usize) -> profiler_snapshot_entry {
        let t = |k: u64| {
            (0..times as u64)
                .map(|i| profiler_time_entry {
                    time_delta: (times as u64 - i) * 997 + k,
                    count: i % 7 + 1,
                })
                .collect::<Vec<_>>()
        };
        profiler_snapshot_entry {
            name: c"render_main_texture".as_ptr(),
            times: darray(t(0)),
            min_time: 997,
            max_time: times as u64 * 997,
            overall_count: 4 * times as u64,
            times_between_calls: darray(t(13)),
            expected_time_between_calls: 16_667,
            min_time_between_calls: 1_000,
            max_time_between_calls: 40_000,
            overall_between_calls_count: 4 * times as u64,
            children: darray(kids),
        }
    }
    let roots = (0..roots)
        .map(|_| {
            entry(
                (0..children).map(|_| entry(Vec::new(), times)).collect(),
                times,
            )
        })
        .collect();
    Box::into_raw(Box::new(profiler_snapshot {
        roots: darray(roots),
    }))
}

fn profiler_io(criterion: &mut Criterion) {
    use obs_c_oracle::profiler_io as c_io;
    use obs_util::ffi::profiler_io as rs_io;

    let snap = profiler_snapshot(8, 8, 32);
    let dir = std::env::temp_dir().join(format!(
        "port-comparison-profiler-io-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::ffi::CString::new(dir.join("out.csv").to_str().unwrap()).unwrap();

    let mut group = criterion.benchmark_group("profiler_snapshot_dump_csv");
    group.throughput(Throughput::Elements(8 * 9 * 64));
    group.bench_function("rust", |b| {
        // SAFETY: `snap` is a valid snapshot for the whole bench and `file`
        // a C string naming a file in a directory that exists.
        b.iter(|| unsafe { rs_io::profiler_snapshot_dump_csv(black_box(snap), file.as_ptr()) })
    });
    group.bench_function("c", |b| {
        // SAFETY: as above; the oracle reads the same struct layout.
        b.iter(|| unsafe {
            c_io::oracle_profiler_snapshot_dump_csv(black_box(snap.cast()), file.as_ptr())
        })
    });
    group.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

pub fn benchmarks(criterion: &mut Criterion) {
    memory(criterion);
    strings(criterion);
    crc(criterion);
    profiler_io(criterion);
}
